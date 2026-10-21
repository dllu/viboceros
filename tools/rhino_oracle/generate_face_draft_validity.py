"""Check captured 3DFace mesh records through public Rhino geometry validity."""
import gzip,hashlib,json
from pathlib import Path
from .client import OracleClient
HERE=Path(__file__).resolve().parent


def request():
 operations=[]
 for suffix in ['drafts','draft_extras']:
  for row in json.loads((HERE/f'observations/face_create_{suffix}.json').read_text())['results']:
   mesh=row['value']['after_script'][0]['mesh']
   operations.append(dict(op='geometry_validity',id=row['id'],source=dict(type='mesh_records',**mesh)))
 return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=operations)


def main():
 q=request();path=HERE/'fixtures/face_draft_validity.json';path.write_text(json.dumps(q,indent=2)+'\n')
 names=['generate_face_draft_validity.py','geometry_validity_probe.py','client.py','rhino_worker.py','structural_sdk.py']
 digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();sources={n:digest(HERE/n)for n in names}
 response=OracleClient(settings_scheme='VibocerosOracleFaceDraftValidity20261021').run_rhino(q,timeout=180)
 assert sources=={n:digest(HERE/n)for n in names}
 response['provenance']=dict(request_sha256=digest(path),sources=sources)
 (HERE/'observations/face_draft_validity.json').write_text(json.dumps(response,indent=2,allow_nan=False)+'\n')
 for name,sha in sources.items():
  snapshot=HERE/'observations/source_snapshots'/(sha+'.gz')
  if not snapshot.exists():snapshot.write_bytes(gzip.compress((HERE/name).read_bytes(),mtime=0))
 print('Recorded captured 3DFace validity',flush=True)
if __name__=='__main__':main()
