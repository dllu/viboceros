"""Actual Freeform arrays at and between spatial corners, with real history."""
import hashlib,json,sys
from pathlib import Path
from .structural_sdk import validate_operations
HERE=Path(__file__).resolve().parent

def request(suite='corners'):
    base=dict(op='curve_array',path='Polyline',representation='Native',spacing='Items',value=4,orientation='Freeform',base_point=None,cplane=[[0.,0.,0.],[1.,0.,0.],[0.,1.,0.]],reverse_path=False,postselect=False,source_count=1,grouped=False,subcurve=None,pick_point=None,finish='Accept',seed_names=[],edits=[],input='Number')
    patches=[dict(path=path,representation=rep,value=n)for path in['Polyline','SkewPolyline','SkewPolyline2']for rep in['Native','Nurbs','Polycurve']for n in[2,4,5,7]]
    patches += [dict(path=path,reverse_path=True)for path in['Polyline','SkewPolyline','SkewPolyline2']]
    patches += [dict(path='SkewPolyline',base_point=[2.,1.,3.]),dict(path='SkewPolyline',spacing='Distance',value=3.),dict(path='SkewPolyline',spacing='Distance',value=1.5),dict(path='SkewPolyline2',source_count=2,grouped=True,postselect=True),dict(path='Polyline',input='Option',edits=[dict(Items=7)])]
    if suite=='stable':
        patches=[dict(path='SkewPolylineStable',representation=rep,value=n,reverse_path=reverse)for rep in['Native','Nurbs','Polycurve']for n in[4,7]for reverse in[False,True]]
        patches += [dict(path='SkewPolylineStable',base_point=[2.,1.,3.]),dict(path='SkewPolylineStable',spacing='Distance',value=3.),dict(path='SkewPolylineStable',spacing='Distance',value=1.5),dict(path='SkewPolylineStable',source_count=2,grouped=True,postselect=True)]
    elif suite!='corners':raise ValueError('unknown corner suite')
    prefix='stable_frame_array_'if suite=='stable'else'frame_array_'
    q=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=[dict(base,id=prefix+str(i),**p)for i,p in enumerate(patches)]);validate_operations(q);return q

def main():
    from .client import OracleClient
    from unittest.mock import patch
    from . import client
    suite=sys.argv[1]if len(sys.argv)>1 else'corners';name='curve_array_frames'+('_stable'if suite=='stable'else'');q=request(suite);path=HERE/'fixtures'/(name+'.json');path.write_text(json.dumps(q,indent=2)+'\n')
    names=['generate_curve_array_frames.py','curve_array_probe.py','number_token.py','rhino_worker.py','structural_sdk.py','client.py','join_probe.py','merge_edges_probe.py','conic_getter_input.py','managed_mouse_callback.py','snap_environment.py','catenary_point_picker.py','group_picking.py'];digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();sources={n:digest(HERE/n)for n in names};validate=client._validate_response
    def record(r,*args):
        (HERE/'observations'/(name+'-attempt.json')).write_text(json.dumps(r,indent=2)+'\n');return validate(r,*args)
    print('Capturing',len(q['operations']),'Freeform corner arrays',flush=True)
    with patch.object(client,'_validate_response',record):r=OracleClient(settings_scheme='VibocerosOracleArrayFrames20261020').run_rhino(q,timeout=900)
    assert sources=={n:digest(HERE/n)for n in names};r['provenance']=dict(request_sha256=digest(path),sources=sources);(HERE/'observations'/(name+'.json')).write_text(json.dumps(r,indent=2,allow_nan=False)+'\n');(HERE/'observations'/(name+'-attempt.json')).unlink(missing_ok=True);print('Corner arrays recorded',flush=True)
if __name__=='__main__':main()
