"""Capture fixed direct mesh-face command observations."""
import argparse,gzip,hashlib,json
from pathlib import Path
from .face_create_probe import request,variant_request,control_request
from .face_draft_cases import request as draft_request,extra_request,corner_request,corner_control_request,corner_finish_request
HERE=Path(__file__).resolve().parent


def main():
 from .client import OracleClient
 parser=argparse.ArgumentParser(description=__doc__);group=parser.add_mutually_exclusive_group();group.add_argument('--variants',action='store_true');group.add_argument('--controls',action='store_true');group.add_argument('--drafts',action='store_true');group.add_argument('--draft-extras',action='store_true');group.add_argument('--corners',action='store_true');group.add_argument('--corner-controls',action='store_true');group.add_argument('--corner-finish',action='store_true');args=parser.parse_args()
 name='face_create_corner_finish'if args.corner_finish else'face_create_corner_controls'if args.corner_controls else'face_create_corners'if args.corners else'face_create_draft_extras'if args.draft_extras else'face_create_drafts'if args.drafts else'face_create_controls'if args.controls else'face_create_variants'if args.variants else'face_create'
 path=HERE/('fixtures/'+name+'.json');q=corner_finish_request()if args.corner_finish else corner_control_request()if args.corner_controls else corner_request()if args.corners else extra_request()if args.draft_extras else draft_request()if args.drafts else control_request()if args.controls else variant_request()if args.variants else request();path.write_text(json.dumps(q,indent=2)+'\n')
 names=['generate_face_create.py','face_create_probe.py','join_probe.py','merge_edges_probe.py','client.py','rhino_worker.py','structural_sdk.py','face_create_input.py','group_picking.py','face_draft_cases.py','face_draft_input.py']
 digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();sources={name:digest(HERE/name)for name in names}
 print('Capturing 3DFace',flush=True)
 r=OracleClient(settings_scheme='VibocerosOracleFaceCreate'+('CornerFinish'if args.corner_finish else'CornerControls'if args.corner_controls else'Corners'if args.corners else'DraftExtras'if args.draft_extras else'Drafts'if args.drafts else'Controls'if args.controls else'Variants'if args.variants else'')+'20261021').run_rhino(q,timeout=300)
 Path('/tmp/viboceros-face-create-last-response.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
 assert sources=={name:digest(HERE/name)for name in names}
 for row in r['results']:
  if 'error'in row:raise ValueError(row)
  if len([e for e in row['value']['events']if e['name']=='3DFace'])!=1:raise ValueError('incomplete '+row['id'])
 r['provenance']=dict(request_sha256=digest(path),sources=sources)
 (HERE/('observations/'+name+'.json')).write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
 for name,sha in sources.items():
  snapshot=HERE/'observations/source_snapshots'/(sha+'.gz')
  if not snapshot.exists():snapshot.write_bytes(gzip.compress((HERE/name).read_bytes(),mtime=0))
 print('Recorded 3DFace',flush=True)


if __name__=='__main__':main()
