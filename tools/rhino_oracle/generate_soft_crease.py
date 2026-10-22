"""Capture fixed soft-crease recipes and preserve immutable producer versions."""
import argparse
import base64
import gzip
import hashlib
import json
from pathlib import Path
from .soft_crease_probe import request

HERE = Path(__file__).resolve().parent
SOURCES = ['generate_soft_crease.py', 'soft_crease_cases.py', 'soft_crease_chain_cases.py', 'soft_crease_probe.py',
           'soft_crease_followup_cases.py', 'fixtures/soft_crease_followup_assets.json',
           'subd_edit_scene.py', 'subdivide_probe.py', 'subd_box_probe.py',
           'join_probe.py', 'merge_edges_probe.py', 'client.py', 'rhino_worker.py', 'structural_sdk.py',
           'conic_getter_input.py', 'managed_mouse_callback.py', 'snap_environment.py', 'number_token.py',
           'catenary_point_picker.py', 'group_picking.py']


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    from .client import OracleClient
    parser = argparse.ArgumentParser(description=__doc__)
    families = [
        ('main', 'soft_crease', 'Main', dict(discovery=False)),
        ('labels', 'soft_crease_labels_discovery', 'Labels', dict(labels=True)),
        ('extra', 'soft_crease_extra', 'Extra', dict(extra=True)),
        ('precision', 'soft_crease_precision', 'Precision', dict(precision=True)),
        ('numeric_labels', 'soft_crease_numeric_labels', 'NumericLabels', dict(numeric_labels=True)),
        ('chains', 'soft_crease_chains', 'Chains', dict(chains=True)),
        ('followup', 'soft_crease_followup', 'Followup', dict(followup=True)),
        ('anchors', 'soft_crease_anchors', 'Anchors', dict(anchors=True)),
    ]
    group = parser.add_mutually_exclusive_group()
    for name, _, _, _ in families:
        group.add_argument('--' + name.replace('_', '-'), action='store_true')
    args = parser.parse_args()
    _, family, suffix, options = next((entry for entry in families if getattr(args, entry[0])),
                                    ('discovery', 'soft_crease_discovery', 'Discovery', {}))
    output = HERE / 'observations' / (family + '.json')
    if output.exists():
        raise ValueError('observations are immutable; add another family')
    query = request(**options)
    path = HERE / 'fixtures' / (family + '.json')
    path.write_text(json.dumps(query, indent=2) + '\n')
    sources = {name: sha(HERE / name) for name in SOURCES}
    for name, digest in sources.items():
        archive = HERE / 'observations/source_snapshots' / (digest + '.gz')
        if not archive.exists():
            archive.write_bytes(gzip.compress((HERE / name).read_bytes(), mtime=0))
    print('Capturing ' + family + ': ' + str(len(query['operations'])) + ' workflows', flush=True)
    try:
        response = OracleClient(settings_scheme='VibocerosOracleSoftCrease20261022' + suffix).run_rhino(
            query, timeout=1500 if suffix in ('Chains', 'Followup', 'Anchors') else 300)
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
