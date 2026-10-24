"""Qualify stored empty Split objects using public native queries and saved files."""
import argparse
import base64
import hashlib
import json
from pathlib import Path
from . import report_mesh_boolean_commands as baseline
from .qualification_sources import with_shared_sources

ROOT, HERE = baseline.ROOT, baseline.HERE
SOURCES = with_shared_sources(baseline.SOURCES + [
    'docs/empty-meshes.md',
    'tools/rhino_oracle/report_mesh_boolean_empty.py',
    'tools/rhino_oracle/test_mesh_boolean_empty.py',
    'crates/viboceros-document/src/geometry.rs',
    'crates/viboceros-document/src/block_instances.rs',
    'crates/viboceros-document/tests/empty_mesh.rs',
    'crates/viboceros-io/src/three_dm.rs',
    'crates/viboceros-io/src/three_dm/blocks.rs',
    'crates/viboceros-io/src/three_dm_units.rs',
    'crates/viboceros-io/tests/empty_mesh.rs',
    'crates/viboceros-opennurbs/native/viboceros_opennurbs.cpp',
    'crates/viboceros-opennurbs/native/structural_blocks.h',
    'crates/viboceros-command/src/geometry_check/tests.rs',
    'crates/viboceros-command/src/interchange.rs',
    'crates/viboceros-command/src/interchange/tests.rs',
    'src/viewport/clipping.rs', 'src/viewport/extents.rs',
    'src/viewport/display_cache/tests.rs', 'src/viewport/selection.rs',
])


def native():
    version, rows, exploratory, paths = baseline.native(['empty'], (2, 0), SOURCES)
    for name, count in [('boolean_empty_inside', 1), ('boolean_empty_later_stage', 2)]:
        row = rows[name]
        storage = row['empty_storage']
        if len(storage['empty']) != count or any(
                e['valid'] or e['bounds_valid'] or not e['selectable'] or not e['selected']
                for e in storage['empty']):
            raise ValueError('native empty mesh validity, bounds or selection differs')
        if len(storage['acceptance']) != len(row['after']) or any(
                not e['added'] or not e['count_increased'] for e in storage['acceptance']):
            raise ValueError('native archive rejected a stored object')
        meshes = [o['mesh'] for o in row['after'] if 'mesh' in o]
        if storage['reloaded_meshes'] != meshes:
            raise ValueError('native archive changed stored mesh geometry')
        path = HERE/'fixtures/mesh_boolean_empty'/(name+'-result.3dm')
        if path.read_bytes() != base64.b64decode(storage['result_3dm']):
            raise ValueError('native result file differs from its capture')
        paths.add(str(path.relative_to(ROOT)))
    return version, rows, exploratory, paths


def summarize(command_log, getter_log):
    version, expected, _, paths = native()
    commands = baseline.measurements(command_log, 'MESH_BOOLEAN_EMPTY_COMMAND', expected)
    getters = baseline.measurements(getter_log, 'MESH_BOOLEAN_EMPTY_GETTER', expected)
    if any(baseline.gap_names(rows, expected) for rows in [commands, getters]):
        raise ValueError('empty mesh command or getter differs from native storage')
    return dict(scope='zero-vertex zero-face Split results, object history and public 3DM persistence',
        native_version=version, epsilon=baseline.EPSILON,
        command_replayed=len(commands), getter_replayed=len(getters),
        commands=commands, getters=getters,
        pending=['empty mesh input behavior in geometry commands',
                 'native empty-only block definition validity and interactive block commands'],
        sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in sorted(paths)})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command_log', type=Path)
    parser.add_argument('getter_log', type=Path)
    parser.add_argument('--output', type=Path, default=ROOT/'docs/empty-mesh-qualification.json')
    args = parser.parse_args()
    record = summarize(args.command_log.read_text(), args.getter_log.read_text())
    args.output.write_text(json.dumps(record, indent=2, allow_nan=False)+'\n')
    print('Empty mesh storage matches: 2 commands and 2 getters')


if __name__ == '__main__':
    main()
