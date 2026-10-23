"""Capture immutable selective quad triangulation references through RhinoCommon."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
from .client import OracleClient
from .nonplanar_quad_cases import request
HERE=Path(__file__).resolve().parent
FAMILY='nonplanar_quad_conversion_v2'
SOURCES=['generate_nonplanar_quads.py','nonplanar_quad_cases.py','nonplanar_quad_probe.py','mesh_channel_capture.py',
         'client.py','rhino_worker.py','structural_sdk.py','join_probe.py','merge_edges_probe.py']


def main():
    parser=argparse.ArgumentParser(description=__doc__);flags=parser.add_mutually_exclusive_group();flags.add_argument('--ngons',action='store_true');flags.add_argument('--angles',action='store_true');args=parser.parse_args()
    family='nonplanar_quad_angles' if args.angles else 'nonplanar_quad_ngons' if args.ngons else FAMILY
    output=HERE/'observations'/(family+'.json')
    if output.exists():raise ValueError('native observations are immutable; add a family')
    query=HERE/'fixtures'/(family+'.json');payload=request(ngons=args.ngons,angles=args.angles)
    query.write_text(json.dumps(payload,indent=2)+'\n')
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    sources={p:digest(HERE/p)for p in SOURCES}
    for p,sha in sources.items():
        target=HERE/'observations/source_snapshots'/(sha+'.gz')
        if not target.exists():target.write_bytes(gzip.compress((HERE/p).read_bytes(),mtime=0))
    print('Capturing '+str(len(payload['operations']))+' selective quad triangulations',flush=True)
    data=OracleClient(settings_scheme='VibocerosOracleNonplanarQuads20261023'+('Angles'if args.angles else 'Ngons'if args.ngons else 'V2')).run_rhino(payload,timeout=240)
    if sources!={p:digest(HERE/p)for p in SOURCES}:raise ValueError('producer changed during capture')
    data['provenance']=dict(request_sha256=digest(query),sources=sources)
    output.write_text(json.dumps(data,indent=2,allow_nan=False)+'\n')
    print('Recorded '+str(len(data['results']))+' references',flush=True)
if __name__=='__main__':main()
