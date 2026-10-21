"""Capture fixed direct mesh/SubD face command observations."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
from . import face_create_probe as probe
from . import face_draft_cases as drafts
HERE=Path(__file__).resolve().parent
VARIANTS=[
 ('variants','face_create_variants',probe.variant_request,'Variants'),
 ('controls','face_create_controls',probe.control_request,'Controls'),
 ('drafts','face_create_drafts',drafts.request,'Drafts'),
 ('draft-extras','face_create_draft_extras',drafts.extra_request,'DraftExtras'),
 ('corners','face_create_corners',drafts.corner_request,'Corners'),
 ('corner-controls','face_create_corner_controls',drafts.corner_control_request,'CornerControls'),
 ('corner-finish','face_create_corner_finish',drafts.corner_finish_request,'CornerFinish'),
 ('subd','face_create_subd',probe.subd_request,'SubD'),
 ('subd-surface','face_create_subd_surface',probe.subd_surface_request,'SubDSurface'),
 ('subd-append','face_create_subd_append',probe.subd_append_request,'SubDAppend'),
 ('subd-append-extras','face_create_subd_append_extras',probe.subd_append_extra_request,'SubDAppendExtras'),
 ('subd-drafts','face_create_subd_drafts',probe.subd_draft_request,'SubDDrafts'),
 ('subd-normals','face_create_subd_normals',probe.subd_normal_request,'SubDNormals'),
]


def main():
 from .client import OracleClient
 parser=argparse.ArgumentParser(description=__doc__);group=parser.add_mutually_exclusive_group()
 for flag,_,_,_ in VARIANTS:group.add_argument('--'+flag,action='store_true')
 args=parser.parse_args()
 chosen=next(((name,factory,suffix)for flag,name,factory,suffix in VARIANTS if getattr(args,flag.replace('-','_'))),('face_create',probe.request,''))
 name,factory,suffix=chosen
 destination=HERE/('observations/'+name+'.json')
 if destination.exists():raise ValueError('Captured observations are immutable; use a new variant')
 path=HERE/('fixtures/'+name+'.json');q=factory();path.write_text(json.dumps(q,indent=2)+'\n')
 names=['generate_face_create.py','face_create_probe.py','join_probe.py','merge_edges_probe.py','client.py','rhino_worker.py','structural_sdk.py','face_create_input.py','group_picking.py','face_draft_cases.py','face_draft_input.py']
 digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();sources={name:digest(HERE/name)for name in names}
 print('Capturing 3DFace '+suffix,flush=True)
 r=OracleClient(settings_scheme='VibocerosOracleFaceCreate'+suffix+'20261021').run_rhino(q,timeout=300)
 Path('/tmp/viboceros-face-create-last-response.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
 assert sources=={name:digest(HERE/name)for name in names}
 for row in r['results']:
  if 'error'in row:raise ValueError(row)
  if len([e for e in row['value']['events']if e['name']=='3DFace'])!=1:raise ValueError('incomplete '+row['id'])
 r['provenance']=dict(request_sha256=digest(path),sources=sources)
 destination.write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
 for name,sha in sources.items():
  snapshot=HERE/'observations/source_snapshots'/(sha+'.gz')
  if not snapshot.exists():snapshot.write_bytes(gzip.compress((HERE/name).read_bytes(),mtime=0))
 print('Recorded 3DFace '+suffix,flush=True)


if __name__=='__main__':main()
