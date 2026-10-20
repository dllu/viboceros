"""Bounded ArrayCrv public getter, placement and real Undo/Redo fixtures."""
import json,hashlib
from pathlib import Path
from .structural_sdk import validate_operations
HERE=Path(__file__).resolve().parent

def request():
    base=dict(op='curve_array',path='LineX',representation='Native',spacing='Items',value=4,orientation='NoRotation',base_point=None,cplane=[[0.,0.,0.],[1.,0.,0.],[0.,1.,0.]],reverse_path=False,postselect=False,source_count=1,grouped=False,subcurve=None,pick_point=None,finish='Accept',seed_names=[],edits=[],input='Number')
    patches=[dict(path=path,orientation=mode)for path in('LineX','LineZ','Arc','Circle','Nurbs','ClosedNurbs')for mode in('Freeform','Roadlike','Stairlike','NoRotation')]
    patches += [dict(path='Arc',orientation=mode,cplane=[[0.,0.,0.],[0.,0.,1.],[1.,0.,0.]])for mode in('Roadlike','Stairlike')]
    patches += [dict(path='Nurbs',reverse_path=True,orientation=mode)for mode in('Freeform','Roadlike','Stairlike','NoRotation')]
    patches += [dict(path='LineY',pick_point=[0.,9.,0.]),dict(path='Folded',pick_point=[10.,2.,0.]),dict(path='Edge',pick_point=[8.,-2.,0.]),dict(path='Circle',pick_point=[0.,4.,-3.])]
    patches += [dict(base_point=[20.,3.,1.]),dict(path='Arc',base_point=[2.,1.,3.],orientation='Freeform'),dict(spacing='Distance',value=3.),dict(path='Circle',spacing='Distance',value=6.),dict(spacing='Distance',value=20.,finish='CancelAfterValue')]
    patches += [dict(subcurve=range,postselect=post)for range,post in[([.2,.8],False),([.8,.2],True)]]
    patches += [dict(input='Option',edits=[dict(Items=6),dict(Orientation='Roadlike')]),dict(spacing='Distance',value=2.,edits=[dict(Items=4),dict(Distance=2.5)]),dict(input='Option',finish='CancelAfterValue')]
    patches += [dict(finish=finish,postselect=True,base_point=[2.,1.,3.]if finish=='CancelBase'else None)for finish in('CancelGuide','CancelBase','CancelSpacing')]
    patches += [dict(grouped=True,source_count=n,postselect=post,seed_names=['Group03','group02'])for n,post in[(1,False),(2,True),(3,False)]]
    patches += [dict(path='Polyline',orientation='Roadlike'),dict(path='Polyline',orientation='Stairlike')]
    patches += [dict(path='Uneven',pick_point=[90.,0.,0.]),dict(path='Circle',pick_point=[-5.,0.,0.]),dict(path='Circle',base_point=[20.,3.,1.]),dict(path='Circle',source_count=2,grouped=True)]
    patches += [dict(path='PlanarPolyline',orientation=mode,representation=representation)for mode in('Roadlike','Stairlike')for representation in('Native','Nurbs','Polycurve')]
    q=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=[dict(base,id='curve_'+str(i),**patch)for i,patch in enumerate(patches)])
    validate_operations(q);return q

def main():
    from .client import OracleClient
    from . import client
    from unittest.mock import patch
    q=request();path=HERE/'fixtures/curve_array_input.json';path.write_text(json.dumps(q,indent=2)+'\n')
    names=['generate_curve_array.py','curve_array_probe.py','number_token.py','rhino_worker.py','structural_sdk.py','client.py','join_probe.py','merge_edges_probe.py','conic_getter_input.py','managed_mouse_callback.py','snap_environment.py','catenary_point_picker.py','group_picking.py']
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();sources={n:digest(HERE/n)for n in names};validate=client._validate_response
    def record(r,*args):
        (HERE/'observations/curve_array_input-attempt.json').write_text(json.dumps(r,indent=2)+'\n');return validate(r,*args)
    print('Capturing',len(q['operations']),'ArrayCrv workflows',flush=True)
    with patch.object(client,'_validate_response',record):r=OracleClient(settings_scheme='VibocerosOracleCurveArrayInput20261020').run_rhino(q,timeout=900)
    assert sources=={n:digest(HERE/n)for n in names};r['provenance']=dict(request_sha256=digest(path),sources=sources);(HERE/'observations/curve_array_input.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n');(HERE/'observations/curve_array_input-attempt.json').unlink(missing_ok=True);print('Curve arrays recorded',flush=True)
if __name__=='__main__':main()
