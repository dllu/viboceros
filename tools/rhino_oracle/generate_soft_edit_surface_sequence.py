"""Capture repeated SoftEditSrf edits and copies in one undo record."""
import hashlib
import json
import subprocess
from pathlib import Path
from .client import OracleClient
from .structural_sdk import validate_operations
HERE=Path(__file__).resolve().parent

def request():
    folder=HERE/'fixtures/soft_edit_surface_sequence';folder.mkdir(exist_ok=True);operations=[]
    for family in('surface','surface_rational','trimmed_rectangle','trimmed_hole'):
        for mode in('repeat','copy','cancel','toggle'):
            name=family+'_'+mode;moves=[dict(uv=[.2,.3],offset=[0.,0.,1.]),dict(uv=[.7,.8],offset=[-1.,0.,-1.])]
            if mode=='toggle':moves[0]['copy']=False;moves[1]['copy']=True
            operations.append(dict(op='soft_edit_surface_sequence',id=name,family=family,stations=[1./3.],v_fraction=2./3.,distance=[4.,4.],offset=[0.,0.,2.],constraint='None',moves=moves,copy=mode in('copy','toggle'),cancel=mode=='cancel',cancel_after=mode=='cancel',artifact_path=str(folder/(name+'.3dm'))))
    q=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=operations);validate_operations(q);return q

def main():
    q=request();p=HERE/'fixtures/soft_edit_surface_sequence.json';p.write_text(json.dumps(q,indent=2)+'\n')
    r=OracleClient(settings_scheme='VibocerosOracleSoftEditSurfaceSequence20261019').run_rhino(q,timeout=900.)
    assert [v['id']for v in r['results']]==[o['id']for o in q['operations']]and all('error'not in v for v in r['results'])
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    r['provenance']=dict(base_commit=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),request_sha256=digest(p),sources={n:digest(HERE/n)for n in('generate_soft_edit_surface_sequence.py','soft_edit_surface_sequence_probe.py','soft_edit_surface_probe.py','soft_edit_curve_probe.py','insert_kink_probe.py','join_probe.py','merge_edges_probe.py','rhino_worker.py','structural_sdk.py')},fixtures={Path(o['artifact_path']).name:digest(Path(o['artifact_path']))for o in q['operations']})
    (HERE/'observations/soft_edit_surface_sequence.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n');print('Recorded',len(r['results']),'repeated surface workflows',flush=True)
if __name__=='__main__':main()
