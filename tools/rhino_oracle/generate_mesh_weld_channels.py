"""Capture immutable, fixed mesh welding cases with exact public producers."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
from .mesh_weld_channels_cases import request, followup_request, prefixes_request, pairs_request
HERE=Path(__file__).resolve().parent
SOURCES=['generate_mesh_weld_channels.py','mesh_weld_channels_cases.py','mesh_weld_channels_probe.py',
         'client.py','rhino_worker.py','structural_sdk.py']


def main():
    from .client import OracleClient
    parser=argparse.ArgumentParser(description=__doc__)
    modes=parser.add_mutually_exclusive_group()
    modes.add_argument('--followup',action='store_true')
    modes.add_argument('--prefixes',action='store_true')
    modes.add_argument('--pairs',action='store_true')
    args=parser.parse_args()
    family='mesh_weld_channels'+('_pairs' if args.pairs else '_prefixes' if args.prefixes else '_followup' if args.followup else '')
    path=HERE/'fixtures'/(family+'.json')
    output=HERE/'observations'/(family+'.json')
    if output.exists(): raise ValueError('native observations are immutable; add a family')
    query=pairs_request() if args.pairs else prefixes_request() if args.prefixes else followup_request() if args.followup else request()
    path.write_text(json.dumps(query,indent=2)+'\n')
    sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    sources={p:sha(HERE/p) for p in SOURCES}
    for name,digest in sources.items():
        target=HERE/'observations/source_snapshots'/(digest+'.gz')
        if not target.exists():target.write_bytes(gzip.compress((HERE/name).read_bytes(),mtime=0))
    print('Capturing '+str(len(query['operations']))+' mesh welding recipes',flush=True)
    result=OracleClient(settings_scheme='VibocerosOracleMeshWeldChannels20261022'+('Pairs' if args.pairs else 'Prefixes' if args.prefixes else 'Followup' if args.followup else '')).run_rhino(query,timeout=300)
    if sources!={p:sha(HERE/p) for p in SOURCES}:raise ValueError('producer changed during capture')
    result['provenance']=dict(request_sha256=sha(path),sources=sources)
    output.write_text(json.dumps(result,indent=2,allow_nan=False)+'\n')
    print('Recorded '+str(len(result['results']))+' recipes; errors='+str([r['id'] for r in result['results'] if 'error'in r]),flush=True)


if __name__=='__main__': main()
