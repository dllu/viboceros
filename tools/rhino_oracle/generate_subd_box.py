"""Capture immutable public SubDBox command observations."""
import gzip,hashlib,json
from pathlib import Path
from .subd_box_probe import request
HERE=Path(__file__).resolve().parent

def main():
 from .client import OracleClient
 destination=HERE/'observations/subd_box.json'
 if destination.exists():raise ValueError('observations are immutable; use a new capture family')
 path=HERE/'fixtures/subd_box.json';q=request();path.write_text(json.dumps(q,indent=2)+'\n')
 names=['generate_subd_box.py','subd_box_probe.py','join_probe.py','merge_edges_probe.py','client.py','structural_sdk.py','rhino_worker.py']
 digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();sources={name:digest(HERE/name)for name in names}
 print('Capturing SubDBox',flush=True)
 response=OracleClient(settings_scheme='VibocerosOracleSubDBox20261021').run_rhino(q,timeout=300)
 Path('/tmp/viboceros-subd-box-last-response.json').write_text(json.dumps(response,indent=2,allow_nan=False)+'\n')
 assert sources=={name:digest(HERE/name)for name in names}
 for row in response['results']:
  if 'error'in row:raise ValueError(row)
  v=row['value']
  if not v['input'].get('cancel')and not v['input'].get('unsupported')and (not v['success']or not any('subd'in o for o in v['after_script'])):raise ValueError('native command failed: '+row['id']+' '+v['history'])
 response['provenance']=dict(request_sha256=digest(path),sources=sources)
 destination.write_text(json.dumps(response,indent=2,allow_nan=False)+'\n')
 for name,sha in sources.items():
  snapshot=HERE/'observations/source_snapshots'/(sha+'.gz')
  if not snapshot.exists():snapshot.write_bytes(gzip.compress((HERE/name).read_bytes(),mtime=0))
 print('Recorded SubDBox',flush=True)
if __name__=='__main__':main()
