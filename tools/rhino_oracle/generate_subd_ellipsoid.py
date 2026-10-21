"""Capture immutable public SubDEllipsoid construction inputs."""
import argparse,gzip,hashlib,json
from pathlib import Path
from .subd_ellipsoid_probe import request
HERE=Path(__file__).resolve().parent

def main():
 from .client import OracleClient
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--discovery',action='store_true');parser.add_argument('--options',action='store_true');parser.add_argument('--shapes',action='store_true');parser.add_argument('--details',action='store_true');args=parser.parse_args()
 family='subd_ellipsoid_details'if args.details else'subd_ellipsoid_shapes'if args.shapes else'subd_ellipsoid_options'if args.options else'subd_ellipsoid_discovery'if args.discovery else'subd_ellipsoid'
 destination=HERE/'observations'/(family+'.json')
 if destination.exists():raise ValueError('observations are immutable; use a new capture family')
 path=HERE/'fixtures'/(family+'.json');q=request(args.discovery,args.options,args.shapes,args.details);path.write_text(json.dumps(q,indent=2)+'\n')
 names=['generate_subd_ellipsoid.py','subd_ellipsoid_probe.py','subd_box_probe.py','join_probe.py','merge_edges_probe.py','client.py','structural_sdk.py','rhino_worker.py']
 digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();sources={name:digest(HERE/name)for name in names}
 print('Capturing '+family,flush=True)
 response=OracleClient(settings_scheme='VibocerosOracle'+('SubDEllipsoidDetailsV2'if args.details else'SubDEllipsoidShapes'if args.shapes else'SubDEllipsoidOptions'if args.options else'SubDEllipsoidDiscovery'if args.discovery else'SubDEllipsoidMain')+'20261021').run_rhino(q,timeout=300)
 Path('/tmp/viboceros-subd-ellipsoid-last-response.json').write_text(json.dumps(response,indent=2,allow_nan=False)+'\n')
 assert sources=={name:digest(HERE/name)for name in names}
 for row in response['results']:
  if 'error'in row:raise ValueError(row)
  v=row['value']
  if not args.discovery and not args.options and not args.shapes:
   if 'Unknown command:'in v['history']and not v['input'].get('unsupported'):raise ValueError('native options failed: '+row['id']+' '+v['history'])
   if not v['input'].get('cancel')and not v['input'].get('unsupported')and not v['input'].get('no_output')and(not v['success']or not any('subd'in o for o in v['after_script'])):raise ValueError('native command failed: '+row['id']+' '+v['history'])
 response['provenance']=dict(request_sha256=digest(path),sources=sources)
 destination.write_text(json.dumps(response,indent=2,allow_nan=False)+'\n')
 for name,sha in sources.items():
  snapshot=HERE/'observations/source_snapshots'/(sha+'.gz')
  if not snapshot.exists():snapshot.write_bytes(gzip.compress((HERE/name).read_bytes(),mtime=0))
 print('Recorded '+family,flush=True)
if __name__=='__main__':main()
