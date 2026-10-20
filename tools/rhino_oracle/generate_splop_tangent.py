"""Native tangent sphere references from acknowledged input, in bounded batches."""
import copy,hashlib,json
from pathlib import Path
from .client import OracleClient
from .structural_sdk import validate_operations
HERE=Path(__file__).resolve().parent
XY=[[0.,0.,0.],[1.,0.,0.],[0.,1.,0.]]

def recipes():
    seeds=[]
    for profile in ('getters','special','curves','spatial'):
        for seed in json.loads((HERE/'fixtures'/('circle_tangent_'+profile+'.json')).read_text())['operations']:
            seed=copy.deepcopy(seed);seed['actions']=seed['actions'][seed['actions'].index('Tangent'):];seeds.append(seed)
    surface=dict(type='surface',degree_u=1,degree_v=1,control_points=[dict(point=p,weight=1.)for p in [[0.,0.,0.],[12.,0.,0.],[0.,10.,0.],[12.,10.,0.]]],control_point_count_u=2,control_point_count_v=2,knots_u=[0.,0.,1.,1.],knots_v=[0.,0.,1.,1.])
    vertical=dict(type='line',start=[-4.,-2.,0.],end=[-4.,12.,0.])
    for edge,pick in [(0,[3.,0.,0.]),(2,[3.,10.,0.])]:
        seeds.append(dict(id='surface_edge_'+str(edge),curves=[surface,vertical],actions=['Tangent',dict(curve=0,edge=edge,pick=pick),6.,dict(curve=1,pick=[-4.,4.,0.])]))
    parabola=dict(type='nurbs',degree=2,control_points=[dict(point=p,weight=1.)for p in[[0.,0.,0.],[4.,0.,0.],[8.,8.,0.]]],knots=[0.,0.,0.,1.,1.,1.])
    upper=dict(type='line',start=[-4.,10.,0.],end=[12.,10.,0.])
    for mode in ('radius','fixed'):
        seeds.append(dict(id='parabola_line_'+mode,curves=[parabola,upper],actions=['Tangent',dict(curve=0,pick=[4.,2.,0.]),3. if mode=='radius'else'FromFirstPoint',dict(curve=1,pick=[4.,10.,0.])]))
    seeds.append(dict(id='arc_line_radius',curves=[dict(type='arc',points=[[3.,0.,0.],[3.*2**-.5,3.*2**-.5,0.],[0.,3.,0.]]),dict(type='line',start=[-4.,6.,0.],end=[10.,6.,0.])],actions=['Tangent',dict(curve=0,pick=[2.,2.,0.]),2.,dict(curve=1,pick=[3.,6.,0.])]))
    return seeds

def request():
    ops=[]
    for seed in recipes():
        cancel=seed['actions'][-1]=='Cancel'
        for surface,flags in [('Plane',{}),('Cylinder',{})]+([]if cancel else[('Cylinder',dict(copy=True,rigid=True,flip=True))]):
            op=dict(op='splop_tangent',id='tangent_'+str(len(ops)),curves=seed.get('curves',[]),actions=seed['actions'],construction_plane=seed.get('construction_plane',XY),surface=surface,copy=False,rigid=False,flip=False,finish='CancelReference'if cancel else'Complete');op.update(flags);ops.append(op)
    for seed in [recipes()[0],next(s for s in recipes()if s['id']=='circle-tangent-tilted-fixed')]:
        ops.append(dict(op='splop_tangent',id='tangent_'+str(len(ops)),curves=seed.get('curves',[]),actions=seed['actions'],construction_plane=seed.get('construction_plane',XY),surface='Cylinder',copy=True,rigid=False,flip=False,finish='CancelCopies'))
    for i in range(3):
        op=copy.deepcopy(ops[48]);op['id']='gap_'+str(i)
        if i==0:op['curves']=list(reversed(op['curves']));op['actions']=['Tangent',dict(curve=0,pick=[1.,8.,0.]),dict(curve=1,pick=[1.2,1.6,0.]),'Enter']
        elif i==1:op['actions'].insert(2,'FromFirstPoint');op['actions'].pop()
        else:op['curves'][1]['start'][1]=-8.;op['curves'][1]['end'][1]=-8.;op['actions'][1]['pick']=[1.2,-1.6,0.];op['actions'][2]['pick']=[1.,-8.,0.]
        ops.append(op)
    q=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=ops);validate_operations(q);return q

def main():
    q=request();path=HERE/'fixtures/splop_tangent.json';path.write_text(json.dumps(q,indent=2)+'\n');print('Capturing tangent sphere cases',len(q['operations']),flush=True)
    names=['generate_splop_tangent.py','client.py','splop_tangent_probe.py','conic_getter_input.py','circle_around_probe.py','getter_recipe.py','curve_source_bounds.py','geometry_validity_probe.py','managed_mouse_callback.py','snap_environment.py','join_probe.py','merge_edges_probe.py','rhino_worker.py','structural_sdk.py','catenary_point_picker.py','group_picking.py'];digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();sources={n:digest(HERE/n)for n in names};rows=[];batches=[]
    for offset in range(0,len(q['operations']),48):
        batch=dict(q,operations=q['operations'][offset:offset+48]);r=OracleClient(settings_scheme='VibocerosOracleSplopTangent20261020').run_rhino(batch,timeout=1200.)
        assert sources=={n:digest(HERE/n)for n in names}
        assert len(r['results'])==len(batch['operations'])and all('error'not in row for row in r['results'])
        batches.append(dict(offset=offset,request=batch,engine_version=r['engine_version']));rows.extend(r['results']);print('Recorded tangent cases',len(rows),flush=True)
    r['results']=rows;r['provenance']=dict(request_sha256=digest(path),sources=sources,batches=batches)
    (HERE/'observations/splop_tangent.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    print('Recorded tangent reference spheres',len(rows),flush=True)
if __name__=='__main__':main()
