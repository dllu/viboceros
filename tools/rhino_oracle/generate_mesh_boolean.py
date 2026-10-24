"""Capture immutable mesh Boolean SDK observations with frozen producers."""
import gzip
import argparse
import base64
import hashlib
import json
from pathlib import Path
from .client import OracleClient
from .mesh_boolean_cases import request

HERE = Path(__file__).resolve().parent
SOURCES = ['generate_mesh_boolean.py', 'mesh_boolean_cases.py', 'mesh_boolean_probe.py',
           'mesh_boolean_command_cases.py', 'mesh_boolean_command_probe.py', 'join_probe.py',
           'mesh_boolean_option_cases.py',
           'client.py', 'rhino_worker.py', 'structural_sdk.py', 'merge_edges_probe.py']


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    flags = parser.add_mutually_exclusive_group()
    for family in ['controls', 'commands', 'lifecycle', 'options', 'memory', 'syntax', 'tolerance', 'policy']:
        flags.add_argument('--'+family, dest='family', action='store_const', const=family)
    parser.set_defaults(family='sdk')
    args = parser.parse_args()
    family = 'mesh_boolean_'+args.family
    output = HERE / 'observations' / (family+'.json')
    if output.exists():
        raise ValueError('native observations are immutable; add a family')
    if args.family in ('commands', 'lifecycle', 'options', 'memory', 'syntax', 'tolerance', 'policy'):
        from .mesh_boolean_command_cases import request as command_request
        payload = command_request(args.family)
    else:
        payload = request(controls=args.family == 'controls')
    query = HERE / 'fixtures' / (family+'.json')
    query.write_text(json.dumps(payload, indent=2)+'\n')
    digest = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
    sources = {name: digest(HERE / name) for name in SOURCES}
    for name, sha in sources.items():
        archive = HERE / 'observations/source_snapshots' / (sha+'.gz')
        if not archive.exists():
            archive.write_bytes(gzip.compress((HERE / name).read_bytes(), mtime=0))
    print('Capturing {} mesh Boolean workflows'.format(len(payload['operations'])), flush=True)
    scheme = 'VibocerosOracleMeshBoolean20261024' + args.family.capitalize()
    data = OracleClient(settings_scheme=scheme).run_rhino(payload, timeout=240)
    if sources != {name: digest(HERE / name) for name in SOURCES}:
        raise ValueError('producer changed during capture')
    data['provenance'] = dict(request_sha256=digest(query), sources=sources)
    output.write_text(json.dumps(data, indent=2, allow_nan=False)+'\n')
    if args.family in ('commands', 'lifecycle', 'options', 'memory', 'syntax', 'tolerance', 'policy'):
        folder = HERE/'fixtures'/family
        folder.mkdir(exist_ok=True)
        for row in data['results']:
            (folder/(row['id']+'.3dm')).write_bytes(base64.b64decode(row['value']['source_3dm']))
    print('Recorded {} mesh Boolean workflows'.format(len(data['results'])), flush=True)


if __name__ == '__main__':
    main()
