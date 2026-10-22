"""Capture fixed soft-crease recipes and preserve immutable producer versions."""
import argparse
import base64
import gzip
import hashlib
import json
from pathlib import Path
from .soft_crease_probe import request

HERE = Path(__file__).resolve().parent
SOURCES = ['generate_soft_crease.py', 'soft_crease_cases.py', 'soft_crease_probe.py',
           'subd_edit_scene.py', 'subdivide_probe.py', 'subd_box_probe.py',
           'join_probe.py', 'merge_edges_probe.py', 'client.py', 'rhino_worker.py', 'structural_sdk.py',
           'conic_getter_input.py', 'managed_mouse_callback.py', 'snap_environment.py', 'number_token.py',
           'catenary_point_picker.py', 'group_picking.py']


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    from .client import OracleClient
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--main', action='store_true')
    parser.add_argument('--labels', action='store_true')
    parser.add_argument('--extra', action='store_true')
    parser.add_argument('--precision', action='store_true')
    parser.add_argument('--numeric-labels', action='store_true')
    args = parser.parse_args()
    if sum([args.main, args.labels, args.extra, args.precision, args.numeric_labels]) > 1:
        parser.error('choose one fixed family')
    family = 'soft_crease_numeric_labels' if args.numeric_labels else 'soft_crease_precision' if args.precision else 'soft_crease_extra' if args.extra else 'soft_crease_labels_discovery' if args.labels else 'soft_crease' if args.main else 'soft_crease_discovery'
    output = HERE / 'observations' / (family + '.json')
    if output.exists():
        raise ValueError('observations are immutable; add another family')
    query = request(discovery=not args.main, labels=args.labels, extra=args.extra, precision=args.precision, numeric_labels=args.numeric_labels)
    path = HERE / 'fixtures' / (family + '.json')
    path.write_text(json.dumps(query, indent=2) + '\n')
    sources = {name: sha(HERE / name) for name in SOURCES}
    for name, digest in sources.items():
        archive = HERE / 'observations/source_snapshots' / (digest + '.gz')
        if not archive.exists():
            archive.write_bytes(gzip.compress((HERE / name).read_bytes(), mtime=0))
    print('Capturing ' + family + ': ' + str(len(query['operations'])) + ' workflows', flush=True)
    try:
        response = OracleClient(settings_scheme='VibocerosOracleSoftCrease20261022' + ('NumericLabels' if args.numeric_labels else 'Precision' if args.precision else 'Extra' if args.extra else 'Labels' if args.labels else 'Main' if args.main else 'Discovery')).run_rhino(query, timeout=300)
    except Exception as error:
        response = dict(error=str(error), excluded_from_qualification=True)
    if sources != {name: sha(HERE / name) for name in SOURCES}:
        raise ValueError('producer changed during capture')
    response['provenance'] = dict(request_sha256=sha(path), sources=sources)
    output.write_text(json.dumps(response, indent=2, allow_nan=False) + '\n')
    for row in response.get('results', []):
        for key, directory in [('source_3dm', 'source'), ('result_3dm', 'result')]:
            if row.get('value', {}).get(key):
                folder = HERE / 'fixtures' / family / directory
                folder.mkdir(parents=True, exist_ok=True)
                (folder / (row['id'] + '.3dm')).write_bytes(base64.b64decode(row['value'][key]))
    if 'error' in response:
        raise ValueError(response['error'])
    errors = [row for row in response['results'] if 'error' in row]
    print('Recorded ' + str(len(response['results'])) + ' rows; ' + str(len(errors)) + ' errors', flush=True)


if __name__ == '__main__':
    main()
