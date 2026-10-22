"""Capture immutable component-unwelding channels through public Rhino APIs."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
from .client import OracleClient
from .mesh_component_unweld_cases import request
HERE=Path(__file__).resolve().parent
SOURCES=['generate_mesh_component_unweld.py','mesh_component_unweld_cases.py',
         'mesh_component_unweld_probe.py','mesh_unweld_channels_cases.py','mesh_channel_capture.py',
         'client.py','rhino_worker.py','structural_sdk.py',
         'mesh_component_unweld_command_cases.py','mesh_component_unweld_command_probe.py','mesh_component_unweld_primed_cases.py','mesh_component_unweld_verified_cases.py','mesh_component_unweld_holdout_cases.py']


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    modes=parser.add_mutually_exclusive_group()
    modes.add_argument('--commands',action='store_true')
    modes.add_argument('--primed',action='store_true')
    modes.add_argument('--verified',action='store_true')
    modes.add_argument('--holdout',action='store_true')
    args=parser.parse_args()
    from .mesh_component_unweld_command_cases import request as commands_request
    from .mesh_component_unweld_primed_cases import request as primed_request
    from .mesh_component_unweld_verified_cases import request as verified_request
    from .mesh_component_unweld_holdout_cases import request as holdout_request
    make=holdout_request if args.holdout else verified_request if args.verified else primed_request if args.primed else commands_request if args.commands else request
    family='mesh_component_unweld_holdout' if args.holdout else 'mesh_component_unweld_verified' if args.verified else 'mesh_component_unweld_primed' if args.primed else 'mesh_component_unweld_commands' if args.commands else 'mesh_component_unweld_channels'
    query=HERE/'fixtures'/(family+'.json')
    output=HERE/'observations'/(family+'.json')
    if output.exists():raise ValueError('native observations are immutable; add a family')
    query.write_text(json.dumps(make(),indent=2)+'\n')
    sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    sources={p:sha(HERE/p) for p in SOURCES}
    for name,digest in sources.items():
        target=HERE/'observations/source_snapshots'/(digest+'.gz')
        if not target.exists():target.write_bytes(gzip.compress((HERE/name).read_bytes(),mtime=0))
    print('Capturing '+str(len(make()['operations']))+' component unwelding recipes',flush=True)
    result=OracleClient(settings_scheme='VibocerosOracleComponentUnweld20261022'+('Holdout'if args.holdout else 'Verified'if args.verified else 'Primed'if args.primed else 'Commands'if args.commands else 'SDK')).run_rhino(make(),timeout=300)
    if sources!={p:sha(HERE/p) for p in SOURCES}:raise ValueError('producer changed during capture')
    result['provenance']=dict(request_sha256=sha(query),sources=sources)
    output.write_text(json.dumps(result,indent=2,allow_nan=False)+'\n')
    print('Recorded '+str(len(result['results']))+' recipes; errors='+str([r['id']for r in result['results']if 'error'in r]),flush=True)


if __name__=='__main__':main()
