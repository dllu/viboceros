"""Audit split vertex-channel samples and explicit source/history ownership."""
import argparse
import hashlib
import json
from pathlib import Path
from . import mesh_channel_comparison as channels
from . import mesh_split_evidence as evidence
from . import report_mesh_split as baseline
from .qualification_sources import with_shared_sources

ROOT = baseline.ROOT
GAPS = ['mesh_split_channels_seam_coplanar_post', 'mesh_split_channels_seam_coplanar_pre',
        'mesh_split_channels_seam_folded']
SOURCES = with_shared_sources(baseline.SOURCES + [
    'crates/viboceros-geometry/src/mesh.rs',
    'crates/viboceros-geometry/src/mesh/closest.rs',
    'crates/viboceros-geometry/src/point/distance_key.rs',
    'crates/viboceros-geometry/src/exact_scalar.rs',
    'tools/rhino_oracle/mesh_channel_comparison.py',
    'tools/rhino_oracle/mesh_split_evidence.py',
    'tools/rhino_oracle/report_mesh_split_channels.py',
    'tools/rhino_oracle/test_mesh_split_channels.py',
    'docs/mesh-split-channels.md',
])


def native():
    return evidence.native(['channels', 'ngon_policy'], SOURCES)


def compare_stage(actual, expected):
    if len(actual) != len(expected):
        return dict(matched=False, objects=[])
    objects = []
    for a, n in zip(actual, expected):
        if 'mesh' not in a and 'mesh' not in n:
            continue
        if 'mesh' not in a or 'mesh' not in n:
            return dict(matched=False, objects=[])
        objects.append(channels.compare(a['mesh'], n['mesh']))
    return dict(matched=all(o['matched'] for o in objects), objects=objects)


def measurements(log, prefix, expected):
    rows = baseline.measurements(log, prefix, expected)
    for row in rows:
        for stage in ['after', 'undo', 'redo']:
            computed = compare_stage(row[stage]['objects'], expected[row['id']][stage])
            if 'channels' in row[stage] and row[stage]['channels'] != computed:
                raise ValueError('mesh channel comparison declaration differs')
            row[stage]['channels'] = computed
    return rows


def summarize(command_log, getter_log):
    version, expected, paths = native()
    commands = measurements(command_log, 'MESH_SPLIT_CHANNEL_COMMAND', expected)
    getters = measurements(getter_log, 'MESH_SPLIT_CHANNEL_GETTER', expected)
    def gaps(rows):
        return sorted(r['id'] for r in rows if r['success'] != expected[r['id']]['success'] or any(
            not r[s][k] for s in ['after', 'undo', 'redo'] for k in ['geometry', 'metadata', 'ngons']) or
            any(not r[s]['channels']['matched'] for s in ['after', 'undo', 'redo']))
    if any(gaps(rows) != GAPS for rows in [commands, getters]):
        raise ValueError('mesh channel compatibility gaps changed')
    channel_matched = lambda rows: sum(all(r[s]['channels']['matched'] for s in ['after', 'undo', 'redo']) for r in rows)
    return dict(scope='MeshSplit vertex-channel samples, raw attribute seams, existing n-gon policies and history',
                native_version=version, geometry_epsilon=channels.GEOMETRY_EPSILON,
                channel_epsilon={name: value[1] for name, value in channels.CHANNELS.items()},
                comparison='Ordered geometry and metadata, bidirectional raw-vertex UV/normalized-normal/color samples on target triangles; coincident seam candidates must match all channels together; RGBA opacity is converted from OpenNURBS transparency',
                command_replayed=len(commands), getter_replayed=len(getters),
                command_matched=len(commands)-len(GAPS), getter_matched=len(getters)-len(GAPS),
                command_channels_matched=channel_matched(commands), getter_channels_matched=channel_matched(getters),
                command_compatibility_gaps=GAPS, getter_compatibility_gaps=GAPS,
                pending=['native n-gon outer boundaries on unwelded source seams',
                         'native raw vertex order, triangulation and between-vertex shading fields',
                         'overlapping source faces, extreme scales, near contacts and comparative performance'],
                commands=commands, getters=getters,
                sha256={p: hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in sorted(paths)})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command_log', type=Path)
    parser.add_argument('getter_log', type=Path)
    parser.add_argument('--output', type=Path, default=ROOT/'docs/mesh-split-channels-qualification.json')
    args = parser.parse_args()
    record = summarize(args.command_log.read_text(), args.getter_log.read_text())
    args.output.write_text(json.dumps(record, indent=2, allow_nan=False)+'\n')
    print('MeshSplit channels: {command_channels_matched}/{command_replayed} commands, {getter_channels_matched}/{getter_replayed} getters; {command_matched} match n-gons too'.format(**record))


if __name__ == '__main__':
    main()
