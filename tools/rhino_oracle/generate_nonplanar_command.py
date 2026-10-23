"""Capture immutable native nonplanar-quad workflows and exact input archives."""
import base64
import argparse
import gzip
import hashlib
import json
from pathlib import Path
from .client import OracleClient
from .nonplanar_command_cases import request
HERE=Path(__file__).resolve().parent
SOURCES=['generate_nonplanar_command.py','nonplanar_command_cases.py','nonplanar_command_probe.py',
    'crease_probe.py','crease_cases.py','subdivide_probe.py','subd_box_probe.py',
    'join_probe.py','merge_edges_probe.py','client.py','rhino_worker.py','structural_sdk.py']


def main():
    parser=argparse.ArgumentParser(description=__doc__);flags=parser.add_mutually_exclusive_group();flags.add_argument('--options',action='store_true');flags.add_argument('--syntax',action='store_true');flags.add_argument('--main',action='store_true');flags.add_argument('--selection',action='store_true');args=parser.parse_args()
    family='nonplanar_command_selection'if args.selection else'nonplanar_command_main'if args.main else'nonplanar_command_syntax_v2'if args.syntax else'nonplanar_command_options'if args.options else'nonplanar_command_discovery'
    output=HERE/'observations'/(family+'.json')
    if output.exists():raise ValueError('native observations are immutable; add a family')
    query=HERE/'fixtures'/(family+'.json');payload=request(options=args.options,syntax=args.syntax,main=args.main,selection=args.selection)
    query.write_text(json.dumps(payload,indent=2)+'\n')
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    sources={p:digest(HERE/p)for p in SOURCES}
    for p,sha in sources.items():
        target=HERE/'observations/source_snapshots'/(sha+'.gz')
        if not target.exists():target.write_bytes(gzip.compress((HERE/p).read_bytes(),mtime=0))
    print('Capturing '+str(len(payload['operations']))+' native nonplanar quad commands',flush=True)
    data=OracleClient(settings_scheme='VibocerosOracleNonplanarCommand20261023'+('Selection'if args.selection else'Main'if args.main else'Syntax'if args.syntax else'Options'if args.options else'Discovery')).run_rhino(payload,timeout=240)
    if sources!={p:digest(HERE/p)for p in SOURCES}:raise ValueError('producer changed during capture')
    data['provenance']=dict(request_sha256=digest(query),sources=sources)
    output.write_text(json.dumps(data,indent=2,allow_nan=False)+'\n')
    folder=HERE/'fixtures'/family;folder.mkdir(exist_ok=True)
    for row in data['results']:
        if row.get('value',{}).get('source_3dm'):
            (folder/(row['id']+'.3dm')).write_bytes(base64.b64decode(row['value']['source_3dm']))
    print('Recorded '+str(len(data['results']))+' native commands',flush=True)
if __name__=='__main__':main()
