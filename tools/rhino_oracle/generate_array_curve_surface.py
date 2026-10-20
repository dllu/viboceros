"""Bounded native ArrayCrvOnSrf getter scenes in one private owned document."""
import json,hashlib
from pathlib import Path
from .structural_sdk import validate_operations
HERE=Path(__file__).resolve().parent

def request():
    base=dict(op='array_curve_surface',scene='Plane',path='LineY',mode='Divide',value=None,
        base_point=[0.,0.,0.],cplane=[[0.,0.,0.],[1.,0.,0.],[0.,1.,0.]],reverse_path=False,
        reverse_face=False,postselect=False,source_count=1,grouped=False,finish='Cancel',
        subcurve=None,seed_names=[],pick_point=None)
    old_patches=[dict(value=3,grouped=True,seed_names=['Group03']),dict(value=3,grouped=True),dict(value=3,pick_point=[0.,9.,5.]),dict(value=3,postselect=True,source_count=2,grouped=True,pick_point=[0.,9.,5.]),dict(path='Polyline',value=8),dict(path='Polyline',mode='Positions',value=[[4.,0.,5.],[4.,4.,5.]]),dict(path='Circle',mode='Multiple',value=[8,10.]),dict(mode='Multiple',value=[1,3.]),dict(mode='Sequence',value=[dict(point=[0.,8.,5.]),dict(multiple=[3,-2.])]),dict(mode='Sequence',value=[dict(divide=3),dict(distance=-2.)]),dict(mode='Distances',value=[12.]),dict(subcurve=[.2,.8],value=3,pick_point=[0.,1.,5.]),dict(value=3,grouped=True,seed_names=['Group01','Group03'])]
    patches=[old_patches[i] for i in [2,3,11,0,1,4,5,6,7,8,9,10,12]]
    patches += [dict(scene=scene,path=path,value=5,reverse_face=flip,reverse_path=rev) for scene,path in [('Tilted','LineX'),('Cylinder','IsoU'),('Cylinder','IsoV'),('Sphere','IsoU'),('Sphere','IsoV'),('Warp','IsoU'),('Warp','IsoV'),('OffSurface','Nurbs')] for flip,rev in [(False,False),(True,True)]]
    patches += [dict(mode='Multiple',value=[3,3.]),dict(mode='Multiple',value=[3,5.]),dict(mode='Multiple',value=[2,0.]),dict(mode='Positions',value=[[0.,2.,5.],[0.,6.,5.]]),dict(mode='Distances',value=[3.,3.,-2.]),dict(mode='Divide',value=1),dict(subcurve=[.2,.8],value=3),dict(subcurve=[.8,.2],value=3),dict(path='Circle',mode='Distances',value=[32.,-1.]),dict(path='LineY',cplane=[[0.,0.,0.],[0.,1.,0.],[-1.,0.,0.]],base_point=[2.,1.,3.],value=4),dict(path='LineY',cplane=[[0.,0.,0.],[0.,0.,1.],[1.,0.,0.]],value=4),dict(mode='Sequence',value=[dict(point=[0.,2.,5.]),dict(multiple=[3,2.]),dict(distance=1.)]),dict(mode='Sequence',value=[dict(multiple=[5,1.]),dict(divide=None),dict(multiple=[None,2.])]),dict(grouped=True,source_count=3,postselect=True,value=4),dict(grouped=True,source_count=3,value=4)]
    ops=[dict(base,id='array_srf_'+str(i),**patch)for i,patch in enumerate(patches)]
    q=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=ops)
    validate_operations(q);return q

def main():
    from .client import OracleClient
    q=request();path=HERE/'fixtures/array_curve_surface.json';path.write_text(json.dumps(q,indent=2)+'\n')
    names=['array_curve_surface_probe.py','number_token.py','rhino_worker.py','structural_sdk.py','client.py','join_probe.py','merge_edges_probe.py','conic_getter_input.py','managed_mouse_callback.py','snap_environment.py','catenary_point_picker.py','group_picking.py']
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();sources={n:digest(HERE/n)for n in names}
    r=OracleClient(settings_scheme='VibocerosOracleArrayCurveSurfaceReproduce20261020').run_rhino(q,timeout=900)
    assert sources=={n:digest(HERE/n)for n in names}
    assert len(r['results'])==len(q['operations'])and all('error'not in row and not row['value']['input_trace']['errors']for row in r['results'])
    r['provenance']=dict(request_sha256=digest(path),sources=sources)
    (HERE/'observations/array_curve_surface.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    print('Recorded native surface curve arrays',len(r['results']),flush=True)
if __name__=='__main__':main()
