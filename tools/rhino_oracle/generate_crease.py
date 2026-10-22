"""Retain fixed crease commands and immutable producer provenance."""
import argparse
import base64
import gzip
import hashlib
import json
from pathlib import Path

from .crease_probe import request

HERE = Path(__file__).resolve().parent
FAMILIES = dict(main='crease_main_v3', extra='crease_extra', policy='crease_policy',
                mixed='crease_mixed', overlap='crease_overlap')
SOURCES = ['generate_crease.py', 'crease_probe.py', 'crease_cases.py',
           'subdivide_probe.py', 'subd_box_probe.py', 'join_probe.py',
           'merge_edges_probe.py', 'client.py', 'rhino_worker.py', 'structural_sdk.py']


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    from .client import OracleClient

    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group()
    for flag in FAMILIES:
        group.add_argument('--' + flag, dest='family', action='store_const', const=flag)
    args = parser.parse_args()
    family = FAMILIES.get(args.family, 'crease_discovery')
    destination = HERE / 'observations' / (family + '.json')
    if destination.exists():
        raise ValueError('observations are immutable; add another family')
    query = request(**({args.family: True} if args.family else {}))
    path = HERE / 'fixtures' / (family + '.json')
    path.write_text(json.dumps(query, indent=2) + '\n')
    sources = {name: digest(HERE / name) for name in SOURCES}
    print('Capturing ' + family, flush=True)
    scheme = 'VibocerosOracleCrease' + (args.family or 'discovery').title() + '20261021'
    try:
        response = OracleClient(settings_scheme=scheme).run_rhino(query, timeout=300)
    except Exception as error:
        response = dict(error=str(error), excluded_from_qualification=True)
    if sources != {name: digest(HERE / name) for name in SOURCES}:
        raise ValueError('producer changed during capture')
    response['provenance'] = dict(request_sha256=digest(path), sources=sources)
    destination.write_text(json.dumps(response, indent=2, allow_nan=False) + '\n')
    for name, sha in sources.items():
        archive = HERE / 'observations/source_snapshots' / (sha + '.gz')
        if not archive.exists():
            archive.write_bytes(gzip.compress((HERE / name).read_bytes(), mtime=0))
    if args.family:
        folder = HERE / 'fixtures' / family
        folder.mkdir(exist_ok=True)
        for row in response.get('results', []):
            if 'value' in row and row['value'].get('source_3dm'):
                (folder / (row['id'] + '.3dm')).write_bytes(base64.b64decode(row['value']['source_3dm']))
    if 'error' in response:
        raise ValueError(response['error'])
    errors = [row for row in response['results'] if 'error' in row]
    print(f'Recorded {family}: {len(response["results"])} rows; {len(errors)} errors', flush=True)
    for row in errors:
        print(row['id'] + ': ' + str(row['error']), flush=True)


if __name__ == '__main__':
    main()
