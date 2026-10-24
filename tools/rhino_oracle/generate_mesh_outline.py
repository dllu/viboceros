"""Capture immutable public mesh outline references with frozen producers."""
import argparse
import base64
import gzip
import hashlib
import json
from pathlib import Path
from .client import OracleClient
from .mesh_outline_cases import FAMILIES, request

HERE = Path(__file__).resolve().parent
SOURCES = ['generate_mesh_outline.py', 'mesh_outline_cases.py', 'mesh_outline_probe.py',
           'mesh_outline_scale_cases.py',
           'mesh_outline_command_cases.py', 'mesh_outline_command_probe.py', 'join_probe.py',
           'client.py', 'rhino_worker.py', 'structural_sdk.py', 'merge_edges_probe.py']
COMMAND_FAMILIES = ('commands', 'lifecycle', 'quads')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    flags = parser.add_mutually_exclusive_group()
    for name in [name for name in FAMILIES if name != 'sdk'] + list(COMMAND_FAMILIES):
        flags.add_argument('--' + name, dest='family', action='store_const', const=name)
    parser.set_defaults(family='sdk')
    args = parser.parse_args()
    family = 'mesh_outline_' + args.family
    output = HERE / 'observations' / (family + '.json')
    if output.exists():
        raise ValueError('native observations are immutable; add a family')
    if args.family in COMMAND_FAMILIES:
        from .mesh_outline_command_cases import request as command_request
        payload = command_request(lifecycle=args.family == 'lifecycle', quads=args.family == 'quads')
    else:
        payload = request(args.family)
    query = HERE / 'fixtures' / (family + '.json')
    query.write_text(json.dumps(payload, indent=2) + '\n')
    digest = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
    sources = {name: digest(HERE / name) for name in SOURCES}
    for name, sha in sources.items():
        archive = HERE / 'observations/source_snapshots' / (sha + '.gz')
        if not archive.exists():
            archive.write_bytes(gzip.compress((HERE / name).read_bytes(), mtime=0))
    print('Capturing {} public mesh outline workflows'.format(len(payload['operations'])), flush=True)
    scheme = 'VibocerosOracleMeshOutline20261023' + args.family.capitalize()
    data = OracleClient(settings_scheme=scheme).run_rhino(payload, timeout=240)
    if sources != {name: digest(HERE / name) for name in SOURCES}:
        raise ValueError('producer changed during capture')
    data['provenance'] = dict(request_sha256=digest(query), sources=sources)
    output.write_text(json.dumps(data, indent=2, allow_nan=False) + '\n')
    if args.family in COMMAND_FAMILIES:
        folder = HERE / 'fixtures' / family
        folder.mkdir(exist_ok=True)
        for row in data['results']:
            (folder / (row['id']+'.3dm')).write_bytes(base64.b64decode(row['value']['source_3dm']))
    print('Recorded {} native workflows'.format(len(data['results'])), flush=True)


if __name__ == '__main__':
    main()
