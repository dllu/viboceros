"""Capture closed surface getters and their atomic model history."""
import hashlib
import json
import subprocess
from pathlib import Path
from .client import OracleClient
from .structural_sdk import validate_operations
HERE=Path(__file__).resolve().parent

def request():
    folder=HERE/'fixtures/soft_edit_surface_closed_sequence';folder.mkdir(exist_ok=True);ops=[]
    for shape in('torus','periodic','sphere','cylinder'):
        for mode in('repeat','copy','toggle'):
            name=shape+'_'+mode;moves=[dict(uv=[.2,.3],offset=[0.,0.,1.]),dict(uv=[.7,.8],offset=[-1.,0.,-1.])]
            if mode=='toggle':moves[0]['copy']=False;moves[1]['copy']=True
            op=dict(op='soft_edit_surface_closed_sequence',id=name,shape=shape,family='surface_rational',stations=[.35],v_fraction=.6,distance=[4.,4.],offset=[.3,-.2,2.],constraint='None',copy=mode in('copy','toggle'),moves=moves,cancel=True,cancel_after=True,artifact_path=str(folder/(name+'.3dm')))
            if shape in('sphere','cylinder'):op['transpose']=True
            if mode=='repeat':op['unfinished']=[.4,.7]
            ops.append(op)
    q=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=ops);validate_operations(q);return q

def main():
    q=request();p=HERE/'fixtures/soft_edit_surface_closed_sequence.json';p.write_text(json.dumps(q,indent=2)+'\n');r=OracleClient(settings_scheme='VibocerosOracleClosedSurfaceSequence20261020').run_rhino(q,timeout=900.)
    assert all('error'not in row for row in r['results'])and len(r['results'])==len(q['operations'])
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();r['provenance']=dict(base_commit=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),request_sha256=digest(p),sources={n:digest(HERE/n)for n in('generate_soft_edit_surface_closed_sequence.py','soft_edit_surface_closed_sequence_probe.py','soft_edit_surface_seams_probe.py','soft_edit_surface_shapes_probe.py','soft_edit_surface_probe.py','soft_edit_curve_probe.py','insert_kink_probe.py','join_probe.py','merge_edges_probe.py','rhino_worker.py','structural_sdk.py')},fixtures={Path(o['artifact_path']).name:digest(Path(o['artifact_path']))for o in q['operations']});(HERE/'observations/soft_edit_surface_closed_sequence.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n');print('Recorded',len(r['results']),'closed surface sequences',flush=True)
if __name__=='__main__':main()
