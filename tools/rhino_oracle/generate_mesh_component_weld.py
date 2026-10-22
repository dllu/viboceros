"""Capture immutable, bounded mesh component-welding command observations."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
from .client import OracleClient
from .mesh_component_weld_cases import request
HERE=Path(__file__).resolve().parent
SOURCES=['generate_mesh_component_weld.py','mesh_component_weld_cases.py',
         'mesh_component_weld_followup_cases.py',
         'mesh_component_weld_holdout_cases.py',
         'mesh_component_weld_probe.py','mesh_channel_capture.py',
         'client.py','rhino_worker.py','structural_sdk.py']


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    modes=parser.add_mutually_exclusive_group()
    modes.add_argument('--followup',action='store_true')
    modes.add_argument('--holdout',action='store_true');args=parser.parse_args()
    from .mesh_component_weld_followup_cases import request as followup_request
    from .mesh_component_weld_holdout_cases import request as holdout_request
    make=holdout_request if args.holdout else followup_request if args.followup else request
    family='mesh_component_weld_holdout'if args.holdout else 'mesh_component_weld_followup'if args.followup else 'mesh_component_weld_commands'
    query=HERE/'fixtures'/(family+'.json')
    output=HERE/'observations'/(family+'.json')
    if output.exists():raise ValueError('native observations are immutable; add a family')
    query.write_text(json.dumps(make(),indent=2)+'\n')
    sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    sources={p:sha(HERE/p)for p in SOURCES}
    for name,digest in sources.items():
        target=HERE/'observations/source_snapshots'/(digest+'.gz')
        if not target.exists():target.write_bytes(gzip.compress((HERE/name).read_bytes(),mtime=0))
    print('Capturing '+str(len(make()['operations']))+' component welding recipes',flush=True)
    result=OracleClient(settings_scheme='VibocerosOracleComponentWeld20261022'+('Holdout'if args.holdout else 'Followup'if args.followup else '')).run_rhino(make(),timeout=300)
    if sources!={p:sha(HERE/p)for p in SOURCES}:raise ValueError('producer changed during capture')
    result['provenance']=dict(request_sha256=sha(query),sources=sources)
    output.write_text(json.dumps(result,indent=2,allow_nan=False)+'\n')
    print('Recorded '+str(len(result['results']))+' recipes; errors='+str([r['id']for r in result['results']if 'error'in r]),flush=True)


if __name__=='__main__':main()
