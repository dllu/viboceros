"""Record SDK transitions while reading the saved repeated-edit fixtures."""
import hashlib
import json
from pathlib import Path
from .client import OracleClient
from .structural_sdk import validate_operations
HERE=Path(__file__).resolve().parent

def request():
    rows=json.loads((HERE/'observations/soft_edit_surface_closed_sequence.json').read_text())['results'];operations=[]
    for row in rows:
        if not row['id'].endswith('_repeat'):continue
        v=row['value'];operations.append(dict(op='soft_edit_surface_repeat_sdk',id=row['id'],artifact_path=v['recipe']['artifact_path'],points=v['all_points'][:6],distance=v['recipe']['distance']))
    q=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=operations);validate_operations(q);return q

def main():
    q=request();p=HERE/'fixtures/soft_edit_surface_repeat_sdk.json';p.write_text(json.dumps(q,indent=2)+'\n');r=OracleClient(settings_scheme='VibocerosOracleSurfaceRepeatSDK20261020').run_rhino(q,timeout=900.)
    assert len(r['results'])==len(q['operations'])and all('error'not in row for row in r['results'])
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();r['provenance']=dict(request_sha256=digest(p),sources={n:digest(HERE/n)for n in ('generate_soft_edit_surface_repeat_sdk.py','soft_edit_surface_repeat_sdk_probe.py','structural_sdk.py','rhino_worker.py')},fixtures={Path(o['artifact_path']).name:digest(Path(o['artifact_path']))for o in q['operations']});(HERE/'observations/soft_edit_surface_repeat_sdk.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n');print('Recorded SDK sequences',len(r['results']),flush=True)
if __name__=='__main__':main()
