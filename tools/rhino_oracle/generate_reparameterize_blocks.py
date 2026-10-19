"""Capture public Reparameterize getters on immutable placed block references.

Run through run_headless.sh exec with PYTHONPATH set to the repository root.
The full response is retained compressed; the replay record omits unchanged
source geometry while retaining independent domains, topology and samples.
"""
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import time
from .client import OracleClient
from .structural_sdk import validate_operations

ROOT = Path(__file__).resolve().parents[2]
FOLDER = ROOT/'tools/rhino_oracle/fixtures/reparameterize_blocks'

def matrix(sx=1., sy=1., sz=1., x=0., y=0., z=0., shear=0.):
    return [[sx,shear,0.,x],[0.,sy,0.,y],[0.,0.,sz,z],[0.,0.,0.,1.]]

def multiply(a,b):
    return [[sum(a[i][k]*b[k][j] for k in range(4)) for j in range(4)]for i in range(4)]

def placed_point(point,placements,path):
    m=placements[0]
    for level in range(len(placements)-1):
        m=multiply(m,multiply(matrix(x=40.*path[level]),placements[level+1]))
    p=[point[0]+30.*path[-1],point[1],point[2],1.]
    return [sum(m[i][j]*p[j]for j in range(4))for i in range(3)]

def request():
    ops=[]
    def add(name,family,point,values,placements=None,path=None,view='Top',**extra):
        placements=placements or [matrix()];path=path or [0]*len(placements)
        actions=[dict(click=placed_point(point,placements,path))]+values
        op=dict(op='reparameterize_block_command',id=name,family=family,placements=placements,
            member_path=path,actions=actions,view=view,artifact_path=str(FOLDER/(name+'.3dm')),display='Shaded')
        op.update(extra);ops.append(op);return op
    add('line_explicit','line',[1.5,2.,0.],[-2.,8.])
    add('circle_scaled_auto','circle',[1.,3.**.5,0.],['Automatic'],[matrix(3.,3.,3.,8.,4.)])
    for name,display,choice in [('box_face_explicit','Shaded',None),('box_wire_first','Wireframe',0),('box_wire_second','Wireframe',1)]:
        o=add(name,'box',[2.,3.,2.],[-2.,8.,10.,30.],[matrix(x=-3.,y=2.)],display=display)
        if choice is not None:o['selection_choice']=choice
    nested=[matrix(-2.,3.,1.,10.,-5.,shear=.4),matrix(.5,2.,1.,4.,3.,shear=.25),matrix(3.,1.,1.,-2.,1.)]
    add('nested_repeated_line','line',[1.5,2.,0.],[-2.,8.],nested,[1,0,1])
    add('nested_line_auto','line',[1.5,2.,0.],['Automatic'],nested,[0,1,0])
    add('reflected_ellipse_auto','circle',[1.,3.**.5,0.],['Automatic'],[matrix(-2.,.5,1.,7.,4.,shear=.3)])
    add('nurbs_shear_explicit','nurbs',[2.125,1.,0.],[-3.,7.],[matrix(1.5,.75,1.,3.,-2.,shear=.4)])
    add('annulus_shear_auto','annulus',[3.,0.,0.],['Automatic'],[matrix(-2.,.75,1.,5.,3.,shear=.2)])
    add('sphere_scale_auto','sphere',[0.,-(3.**.5),1.],['Automatic'],[matrix(3.,3.,3.,2.,5.,1.)],view='Front')
    add('cylinder_reflected_auto','cylinder',[0.,-2.,1.5],['Automatic'],[matrix(-2.,.75,1.5,3.,4.,2.)],view='Front')
    for name,values in [('multi_auto',['Automatic']),('multi_explicit',[-2.,8.])]:
        o=add(name,'line',[1.5,2.,0.],values);o['actions'].insert(1,dict(click=[31.5,2.,0.]))
    for name,picks in [('different_defaults',[[1.5,2.,0.],[31.5,2.,0.]]),('different_reverse_defaults',[[31.5,2.,0.],[1.5,2.,0.]])]:
        o=add(name,'line',[1.5,2.,0.],[-2.,8.],alternate_domain=[-4.,9.]);o['actions']=[dict(click=p)for p in picks]+[-2.,8.]
    o=add('multi_surface','box',[2.,3.,2.],[-2.,8.,10.,30.]);o['actions'].insert(1,dict(click=[32.,3.,2.]))
    o=add('same_leaf_faces','box',[2.,3.,2.],[-2.,8.,10.,30.],display='Wireframe',selection_choices=[0,1]);o['actions'].insert(1,dict(click=[2.,3.,2.]))
    add('block_noop','line',[1.5,2.,0.],['Enter','Enter'])
    add('block_cancel','line',[1.5,2.,0.],['Cancel'])
    o=add('mixed_rejected','line',[1.5,2.,0.],[],alternate_family='box');o['actions'].append(dict(click=[32.,3.,2.]))
    q=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-8,relative=1e-12,angular=1e-8),operations=ops)
    validate_operations(q);return q

def main():
    FOLDER.mkdir(exist_ok=True)
    q=request();prefix=ROOT/'tools/rhino_oracle/observations/reparameterize_blocks'
    prefix.with_suffix('.request.json').write_text(json.dumps(q,indent=2)+'\n')
    started=time.monotonic()
    response=OracleClient(settings_scheme='VibocerosOracleReparameterizeBlocks20261018FinalV1').run_rhino(q,timeout=600.)
    with gzip.open(str(prefix)+'.raw.json.gz','wt')as stream:json.dump(response,stream,indent=2,allow_nan=False)
    if response.get('block_controller_aborts'):raise ValueError('owned input controller aborted')
    records=[]
    for op,row in zip(q['operations'],response['results']):
        value=row['value'];before=value['before'];after=value['after'];ids={o['id']for o in before['objects']}
        outputs=[o for o in after['objects']if o['id']not in ids]
        intact=(before['definitions']==after['definitions'] and all(o in after['objects']for o in before['objects']))
        expected_success=op['id']not in ('block_cancel','mixed_rejected')
        if not intact or value['success']!=expected_success or any(t['errors']for t in value['input']):
            raise ValueError('incomplete block capture '+op['id']+': '+str(value['input']))
        if outputs and (value['undo']!=before or value['redo']!=after):raise ValueError('history mismatch '+op['id'])
        records.append(dict(id=op['id'],success=value['success'],root_id=value['root_id'],member_path=value['member_path'],
            outputs=outputs,history=value['history'],input=value['input'],sources_unchanged=intact,
            undo_restores_source=value['undo']==before if outputs else None,redo_restores_result=value['redo']==after if outputs else None))
    digest=lambda path:hashlib.sha256(path.read_bytes()).hexdigest()
    source_files=['client.py','rhino_worker.py','reparameterize_block_probe.py','reparameterize_block_picker.py','conic_getter_input.py','structural_sdk.py','generate_reparameterize_blocks.py']
    record=dict(protocol_version=1,engine=response['engine'],engine_version=response['engine_version'],results=records,
        provenance=dict(base_commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),seconds=time.monotonic()-started,
            sources={f:digest(Path(__file__).with_name(f))for f in source_files},fixtures={p.name:digest(p)for p in FOLDER.glob('*.3dm') if p.stem in {op['id']for op in q['operations']}}))
    prefix.with_suffix('.json').write_text(json.dumps(record,indent=2,allow_nan=False)+'\n')
    print('Recorded',len(records),'public command cases in',time.monotonic()-started,'seconds',flush=True)

if __name__=='__main__':main()
