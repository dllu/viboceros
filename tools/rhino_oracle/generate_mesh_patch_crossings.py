"""Capture fixed MeshPatch intersections without presuming native success."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
from . import mesh_patch_probe as probe

HERE = Path(__file__).resolve().parent
CAPTURES = {
    '': ('', probe.crossing_request),
    'verify': ('Verify', probe.crossing_verify_request),
    'untrimmed': ('Untrimmed', probe.untrimmed_crossing_request),
    'seeded': ('Seeded', probe.seeded_crossing_request),
    'junctions': ('Junctions', probe.junction_request),
    'audit': ('Audit', probe.audit_request),
    'points': ('Points', probe.point_guide_request),
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group()
    for name in CAPTURES:
        if name:
            group.add_argument('--' + name, action='store_true')
    args = parser.parse_args()
    kind = next((name for name in CAPTURES if name and getattr(args, name)), '')
    scheme, factory = CAPTURES[kind]
    name = 'mesh_patch_crossings' + ('_' + kind if kind else '')
    request = factory()
    path = HERE / ('fixtures/' + name + '.json')
    path.write_text(json.dumps(request, indent=2) + '\n')
    names = ['generate_mesh_patch_crossings.py', 'mesh_patch_probe.py', 'join_probe.py',
             'merge_edges_probe.py', 'client.py', 'rhino_worker.py', 'structural_sdk.py']
    def digest(path):
        return hashlib.sha256(path.read_bytes()).hexdigest()
    sources = {name: digest(HERE / name) for name in names}
    from .client import OracleClient
    print('Capturing MeshPatch ' + kind, flush=True)
    response = OracleClient(settings_scheme='VibocerosOracleMeshPatchCrossings' + scheme + '20261020').run_rhino(request, timeout=300)
    Path('/tmp/viboceros-' + name + '-last-response.json').write_text(json.dumps(response, indent=2, allow_nan=False) + '\n')
    assert sources == {name: digest(HERE / name) for name in names}
    for row in response['results']:
        if 'error' in row:
            raise ValueError(row)
        events = [event for event in row['value']['events'] if event['name'] == 'MeshPatch']
        if len(events) != 1 or events[0]['result'] not in ['Success', 'Failure']:
            raise ValueError('incomplete command ' + row['id'])
    response['provenance'] = dict(request_sha256=digest(path), sources=sources)
    (HERE / ('observations/' + name + '.json')).write_text(json.dumps(response, indent=2, allow_nan=False) + '\n')
    for name, sha in sources.items():
        snapshot = HERE / 'observations/source_snapshots' / (sha + '.gz')
        if not snapshot.exists():
            snapshot.write_bytes(gzip.compress((HERE / name).read_bytes(), mtime=0))
    print('Recorded MeshPatch ' + kind, flush=True)


if __name__ == '__main__':
    main()
