"""Capture closed charts, singular sides and fixed periodic edges without rewrites."""
import hashlib
import json
import subprocess
from pathlib import Path
from .client import OracleClient
from .structural_sdk import validate_operations
HERE=Path(__file__).resolve().parent

def request():
    original=json.loads((HERE/'fixtures/soft_edit_surface_shapes.json').read_text())
    folder=HERE/'fixtures/soft_edit_surface_seams';folder.mkdir(exist_ok=True);ops=[]
    for source in original['operations']:
        shape=source['shape']
        if shape=='torus'or shape=='periodic'and not source.get('transpose')or shape in('sphere','cylinder')and source.get('transpose'):
            op=dict(source,op='soft_edit_surface_seams');op['artifact_path']=str(folder/(op['id']+'.3dm'));ops.append(op)
    def add(name,shape='torus',**kw):
        ops.append(dict(op='soft_edit_surface_seams',id=name,shape=shape,family='surface_rational',stations=[kw.pop('u',.35)],v_fraction=kw.pop('v',.6),distance=kw.pop('distance',[4.,4.]),offset=kw.pop('offset',[.3,-.2,2.]),cancel=True,cancel_after=True,artifact_path=str(folder/(name+'.3dm')),**kw))
    for f in(.125,.25,.375,.499,.501,.75,.875):add('torus_v_'+str(f).replace('.','_'),v=f)
    for f in(.125,.625,.875):add('torus_u_'+str(f).replace('.','_'),u=f)
    add('torus_wide',distance=[10.,100.]);add('torus_degree',degree_u=5,degree_v=4)
    for fixed in(False,True):
        add('periodic_u_fixed_'+str(int(fixed)),shape='periodic',degree_u=5,transpose=True,fix_edges=fixed)
        add('cylinder_v_fixed_'+str(int(fixed)),shape='cylinder',degree_v=5,transpose=True,fix_edges=fixed)
    for u,v in[(0.,0.),(1.,1.),(0.,.6),(1.,.4)]:add('sphere_pole_'+str(u).replace('.','_')+'_'+str(v).replace('.','_'),shape='sphere',transpose=True,u=u,v=v)
    for v in(.6,.7,.85):add('torus_skew_'+str(v).replace('.','_'),skew_v=True,v=v)
    add('torus_wide_fixed',distance=[10.,100.],fix_edges=True)
    q=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=ops);validate_operations(q);return q

def main():
    q=request();p=HERE/'fixtures/soft_edit_surface_seams.json';p.write_text(json.dumps(q,indent=2)+'\n');r=OracleClient(settings_scheme='VibocerosOracleSurfaceSeams20261020').run_rhino(q,timeout=900.)
    assert all('error'not in row for row in r['results'])and len(r['results'])==len(q['operations'])
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    r['provenance']=dict(base_commit=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),request_sha256=digest(p),sources={n:digest(HERE/n)for n in('generate_soft_edit_surface_seams.py','soft_edit_surface_seams_probe.py','soft_edit_surface_shapes_probe.py','soft_edit_surface_probe.py','soft_edit_curve_probe.py','insert_kink_probe.py','join_probe.py','merge_edges_probe.py','rhino_worker.py','structural_sdk.py')},fixtures={Path(o['artifact_path']).name:digest(Path(o['artifact_path']))for o in q['operations']})
    (HERE/'observations/soft_edit_surface_seams.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n');print('Recorded',len(r['results']),'closed surface validity cases',flush=True)
if __name__=='__main__':main()
