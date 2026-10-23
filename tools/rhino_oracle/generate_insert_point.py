"""Capture immutable mesh/SubD InsertPoint scenes and exact before files."""
import base64
import argparse
import gzip
import hashlib
import json
from pathlib import Path
from .client import OracleClient
from .insert_point_cases import request
HERE=Path(__file__).resolve().parent
SOURCES=['generate_insert_point.py','insert_point_cases.py','insert_point_probe.py','conic_getter_input.py','managed_mouse_callback.py','snap_environment.py','number_token.py','catenary_point_picker.py','group_picking.py','crease_probe.py','crease_cases.py','subdivide_probe.py','subd_box_probe.py','join_probe.py','merge_edges_probe.py','client.py','rhino_worker.py','structural_sdk.py']


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    flags=parser.add_mutually_exclusive_group();flags.add_argument('--live',action='store_true');flags.add_argument('--full',action='store_true');flags.add_argument('--regions',action='store_true');args=parser.parse_args();family='insert_point_regions'if args.regions else'insert_point_full'if args.full else'insert_point_live'if args.live else'insert_point_discovery';query=HERE/'fixtures'/(family+'.json')
    output=HERE/'observations'/(family+'.json')
    if output.exists():raise ValueError('native observations are immutable; add a family')
    payload=request(live=args.live,full=args.full,regions=args.regions)
    query.write_text(json.dumps(payload,indent=2)+'\n')
    sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    sources={p:sha(HERE/p)for p in SOURCES}
    for name,digest in sources.items():
        target=HERE/'observations/source_snapshots'/(digest+'.gz')
        if not target.exists():target.write_bytes(gzip.compress((HERE/name).read_bytes(),mtime=0))
    print('Capturing '+str(len(payload['operations']))+' InsertPoint scenes',flush=True)
    result=OracleClient(settings_scheme='VibocerosOracleInsertPoint20261023'+('Regions'if args.regions else'Full'if args.full else'Live'if args.live else'Discovery')).run_rhino(payload,timeout=240)
    if sources!={p:sha(HERE/p)for p in SOURCES}:raise ValueError('producer changed during capture')
    result['provenance']=dict(request_sha256=sha(query),sources=sources)
    output.write_text(json.dumps(result,indent=2,allow_nan=False)+'\n')
    folder=HERE/'fixtures'/family;folder.mkdir(exist_ok=True)
    for row in result['results']:
        if row.get('value',{}).get('source_3dm'):
            (folder/(row['id']+'.3dm')).write_bytes(base64.b64decode(row['value']['source_3dm']))
    print('Recorded '+str(len(result['results']))+' scenes',flush=True)


if __name__=='__main__':main()
