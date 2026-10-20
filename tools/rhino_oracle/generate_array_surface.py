"""Bounded ArraySrf input, preview, cancellation and geometry fixtures."""
import json,hashlib
from pathlib import Path
from .structural_sdk import validate_operations
HERE=Path(__file__).resolve().parent

def request():
    base=dict(op='array_surface',scene='Plane',counts=[3,3],mode='UV',base_point=[0.,0.,0.],up_point=None,cplane=[[0.,0.,0.],[1.,0.,0.],[0.,1.,0.]],reverse_face=False,postselect=False,source_count=1,grouped=False,preview=True,edits=[],finish='Enter',seed_names=[],domains=[[0.,1.],[0.,1.]],swap_uv=False)
    patches=[dict(scene=scene,mode=mode)for scene in('Plane','Tilted','Warp','Cylinder','Sphere','TrimmedPlane','DoubleWarp')for mode in('UV','Isocurve')]
    patches += [dict(scene=scene,reverse_face=True)for scene in('Plane','Warp','Cylinder')]
    rot=[[0.,0.,0.],[0.,1.,0.],[-1.,0.,0.]]
    patches += [dict(cplane=rot),dict(cplane=rot,up_point=[0.,0.,1.]),dict(cplane=rot,up_point=[1.,2.,3.]),dict(up_point=[1.,2.,3.]),dict(cplane=[[0.,0.,0.],[0.,0.,1.],[1.,0.,0.]]),dict(up_point=[1.,0.,0.]),dict(up_point=[0.,0.,-1.]),dict(base_point=[3.,4.,5.],up_point=[4.,6.,8.])]
    patches += [dict(scene='Cylinder',counts=counts,mode=mode)for counts,mode in[([1,1],'UV'),([4,1],'Isocurve'),([1,4],'Isocurve')]]
    patches += [dict(scene='Warp',edits=[dict(UNumber=4),dict(VNumber=2),dict(Mode='Isocurve')]),dict(scene='Warp',edits=[dict(Mode='Isocurve'),dict(UNumber=4)],finish='Cancel')]
    patches += [dict(finish=finish,postselect=True)for finish in('CancelBase','CancelUp','CancelSurface','CancelU','CancelV')]
    patches += [dict(grouped=True,source_count=n,postselect=post,seed_names=['Group03','group02'])for n,post in[(1,False),(2,True),(3,False)]]
    patches += [dict(scene='Warp',domains=[[-7.,3.],[100.,101.]],swap_uv=True,mode=mode,preview=False)for mode in('UV','Isocurve')]
    patches += [dict(scene='Sphere',counts=counts,mode=mode)for counts,mode in[([1,3],'Isocurve'),([4,3],'UV')]]
    patches += [dict(grouped=True,source_count=n,postselect=post,finish='Cancel')for n,post in[(1,False),(2,True)]]
    patches += [dict(scene='TrimmedPlane',reverse_face=True,swap_uv=True),dict(scene='DoubleWarp',mode='Isocurve',counts=[4,4],domains=[[-7.,3.],[100.,101.]])]
    ops=[dict(base,id='surface_'+str(i),**patch)for i,patch in enumerate(patches)]
    q=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=ops);validate_operations(q);return q

def main():
    from .client import OracleClient
    q=request();path=HERE/'fixtures/array_surface.json';path.write_text(json.dumps(q,indent=2)+'\n')
    names=['generate_array_surface.py','array_surface_probe.py','number_token.py','rhino_worker.py','structural_sdk.py','client.py','join_probe.py','merge_edges_probe.py','conic_getter_input.py','managed_mouse_callback.py','snap_environment.py'];digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();sources={n:digest(HERE/n)for n in names}
    print('Capturing',len(q['operations']),'ArraySrf workflows',flush=True)
    from unittest.mock import patch
    from . import client
    validate=client._validate_response
    def record(response,*args):
        (HERE/'observations/array_surface-attempt.json').write_text(json.dumps(response,indent=2)+'\n')
        return validate(response,*args)
    with patch.object(client,'_validate_response',record):
        r=OracleClient(settings_scheme='VibocerosOracleArraySurface20261020').run_rhino(q,timeout=900)
    assert sources=={n:digest(HERE/n)for n in names};r['provenance']=dict(request_sha256=digest(path),sources=sources);(HERE/'observations/array_surface.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n');(HERE/'observations/array_surface-attempt.json').unlink(missing_ok=True);print('Surface arrays recorded',flush=True)
if __name__=='__main__':main()
