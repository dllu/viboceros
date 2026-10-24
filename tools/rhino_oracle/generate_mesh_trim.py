"""Capture immutable MeshTrim workflows in an owned private Rhino process."""
import argparse
import base64
import gzip
import hashlib
import json
from pathlib import Path
from .client import OracleClient
from .mesh_trim_cases import FAMILIES, request
from .generate_mesh_split import SOURCES as SPLIT_SOURCES

HERE = Path(__file__).resolve().parent
SOURCES = [
    'mesh_trim_getter_control_cases.py', 'mesh_trim_split_control.py',
    'mesh_trim_pick_control_cases.py', 'mesh_trim_pick_trace.py', 'managed_mouse_callback.py',
    'mesh_trim_curve_control_cases.py', 'mesh_trim_cutter_policy_cases.py',
    'mesh_trim_snapshot_policy_cases.py', 'mesh_trim_continuation_cases.py',
    'mesh_trim_picking.py', 'mesh_trim_control_cases.py', 'mesh_trim_input.py',
    'component_picking.py', 'hole_picking.py', 'generate_mesh_trim.py',
    'mesh_trim_cases.py', 'mesh_trim_probe.py',
] + SPLIT_SOURCES


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    flags = parser.add_mutually_exclusive_group()
    flags.add_argument('--family', choices=list(FAMILIES), default='discovery')
    for name in FAMILIES:
        if name != 'discovery':
            flags.add_argument('--'+name.replace('_', '-'), dest='family',
                               action='store_const', const=name)
    args = parser.parse_args()
    family = args.family
    name = 'mesh_trim_' + family
    output = HERE / 'observations' / (name + '.json')
    if output.exists():
        raise ValueError('observations are immutable; add a family')
    query = HERE / 'fixtures' / (name + '.json')
    query.write_text(json.dumps(request(family), indent=2) + '\n')
    digest = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
    sources = {name: digest(HERE / name) for name in SOURCES}
    for sha, path in [(sha, HERE / name) for name, sha in sources.items()]:
        archive = HERE / 'observations/source_snapshots' / (sha + '.gz')
        if not archive.exists():
            archive.write_bytes(gzip.compress(path.read_bytes(), mtime=0))
    print('Capturing MeshTrim workflows', flush=True)
    data = OracleClient(settings_scheme='VibocerosOracleMeshTrim20261024' + family.capitalize()).run_rhino(
        request(family), timeout=240)
    if sources != {name: digest(HERE / name) for name in SOURCES}:
        raise ValueError('producer changed during capture')
    data['provenance'] = dict(request_sha256=digest(query), sources=sources)
    output.write_text(json.dumps(data, indent=2, allow_nan=False) + '\n')
    folder = HERE / 'fixtures' / name
    folder.mkdir(exist_ok=True)
    for row in data['results']:
        (folder / (row['id'] + '.3dm')).write_bytes(base64.b64decode(row['value']['source_3dm']))
    print('Recorded {} MeshTrim workflows'.format(len(data['results'])), flush=True)


if __name__ == '__main__':
    main()
