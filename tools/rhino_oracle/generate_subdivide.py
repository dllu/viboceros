"""Capture immutable public mesh/SubD subdivision inputs."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
from .subdivide_probe import request

HERE = Path(__file__).resolve().parent
FLAGS = ['discovery', 'details', 'orientations', 'extra', 'ngons']
SETTINGS = dict(discovery='DiscoveryV2', details='Details',
                orientations='Orientations', extra='Extra', ngons='Ngons')


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    from .client import OracleClient
    parser = argparse.ArgumentParser(description=__doc__)
    choices = parser.add_mutually_exclusive_group()
    for flag in FLAGS:
        choices.add_argument('--' + flag, action='store_true')
    args = parser.parse_args()
    selected = next((flag for flag in FLAGS if getattr(args, flag)), None)
    family = 'subdivide' + ('_' + selected if selected else '')
    destination = HERE / 'observations' / (family + '.json')
    if destination.exists():
        raise ValueError('observations are immutable; use a new capture family')
    path = HERE / 'fixtures' / (family + '.json')
    query = request(**{flag: getattr(args, flag) for flag in FLAGS})
    path.write_text(json.dumps(query, indent=2) + '\n')
    names = ['generate_subdivide.py', 'subdivide_probe.py', 'subd_box_probe.py',
             'join_probe.py', 'merge_edges_probe.py', 'client.py',
             'structural_sdk.py', 'rhino_worker.py']
    sources = {name: digest(HERE / name) for name in names}
    print('Capturing ' + family, flush=True)
    scheme = 'VibocerosOracleSubDivide' + SETTINGS.get(selected, 'Main') + '20261021'
    response = OracleClient(settings_scheme=scheme).run_rhino(query, timeout=300)
    Path('/tmp/viboceros-subdivide-last-response.json').write_text(
        json.dumps(response, indent=2, allow_nan=False) + '\n')
    if sources != {name: digest(HERE / name) for name in names}:
        raise ValueError('producer changed during capture')
    for row in response['results']:
        if 'error' in row:
            raise ValueError(row)
        value = row['value']
        if not args.discovery:
            for step in value['steps']:
                if 'Unknown command:' in step['history']:
                    raise ValueError('native options failed: ' + row['id'] + ' ' + step['history'])
                if not value['input'].get('cancel') and not step['success']:
                    raise ValueError('native command failed: ' + row['id'] + ' ' + step['history'])
    response['provenance'] = dict(request_sha256=digest(path), sources=sources)
    destination.write_text(json.dumps(response, indent=2, allow_nan=False) + '\n')
    for name, sha in sources.items():
        snapshot = HERE / 'observations/source_snapshots' / (sha + '.gz')
        if not snapshot.exists():
            snapshot.write_bytes(gzip.compress((HERE / name).read_bytes(), mtime=0))
    print('Recorded ' + family, flush=True)


if __name__ == '__main__':
    main()
