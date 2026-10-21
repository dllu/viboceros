"""Capture immutable public SubDSphere styles and sphere inputs."""
import argparse,gzip,hashlib,json
from pathlib import Path
from .subd_sphere_probe import request
HERE=Path(__file__).resolve().parent

def main():
 from .client import OracleClient
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--discovery',action='store_true');parser.add_argument('--styles',action='store_true');args=parser.parse_args()
 family='subd_sphere_styles'if args.styles else'subd_sphere_discovery'if args.discovery else'subd_sphere'
 destination=HERE/'observations'/(family+'.json')
 if destination.exists():raise ValueError('observations are immutable; use a new capture family')
 path=HERE/'fixtures'/(family+'.json');q=request(args.discovery,args.styles);path.write_text(json.dumps(q,indent=2)+'\n')
 names=['generate_subd_sphere.py','subd_sphere_probe.py','subd_box_probe.py','join_probe.py','merge_edges_probe.py','client.py','structural_sdk.py','rhino_worker.py']
 digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();sources={name:digest(HERE/name)for name in names}
 print('Capturing '+family,flush=True)
 response=OracleClient(settings_scheme='VibocerosOracle'+('SubDSphereStyles'if args.styles else'SubDSphereDiscovery'if args.discovery else'SubDSphereMain')+'20261021').run_rhino(q,timeout=300)
 Path('/tmp/viboceros-subd-sphere-last-response.json').write_text(json.dumps(response,indent=2,allow_nan=False)+'\n')
 assert sources=={name:digest(HERE/name)for name in names}
 for row in response['results']:
  if 'error'in row:raise ValueError(row)
  v=row['value']
  if not args.discovery and not args.styles:
   if 'Unknown command:'in v['history']and not v['input'].get('unsupported'):raise ValueError('native options failed: '+row['id']+' '+v['history'])
   if not v['input'].get('cancel')and not v['input'].get('unsupported')and(not v['success']or not any('subd'in o for o in v['after_script'])):raise ValueError('native command failed: '+row['id']+' '+v['history'])
 response['provenance']=dict(request_sha256=digest(path),sources=sources)
 destination.write_text(json.dumps(response,indent=2,allow_nan=False)+'\n')
 for name,sha in sources.items():
  snapshot=HERE/'observations/source_snapshots'/(sha+'.gz')
  if not snapshot.exists():snapshot.write_bytes(gzip.compress((HERE/name).read_bytes(),mtime=0))
 print('Recorded '+family,flush=True)
if __name__=='__main__':main()
