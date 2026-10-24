"""Audit multi-target and Split results against independently captured command scenes."""
import argparse
import hashlib
import json
from pathlib import Path
from . import report_mesh_boolean_commands as baseline
from .qualification_sources import with_shared_sources

HERE, ROOT = baseline.HERE, baseline.ROOT
FAMILIES = ['sets', 'split', 'split_controls', 'split_stages']
SOURCES = with_shared_sources(baseline.SOURCES + [
    'docs/mesh-boolean-sets.md',
    'tools/rhino_oracle/mesh_boolean_sets_cases.py',
    'tools/rhino_oracle/report_mesh_boolean_sets.py',
    'tools/rhino_oracle/test_mesh_boolean_sets.py',
])
GAPS = [
    "boolean_sets_difference_nested_targets_post",
    "boolean_sets_difference_nested_targets_pre",
    "boolean_sets_difference_overlapping_targets_post",
    "boolean_sets_difference_overlapping_targets_pre",
    "boolean_sets_intersection_nested_targets_post",
    "boolean_sets_intersection_nested_targets_pre",
    "boolean_sets_intersection_overlapping_targets_post",
    "boolean_sets_intersection_overlapping_targets_pre",
    "boolean_split_face_touch_post",
    "boolean_split_face_touch_pre",
    "boolean_split_identical_post",
    "boolean_split_identical_pre"
]


def native():
    return baseline.native(FAMILIES, (67, 2), SOURCES)


def summarize(command_log, getter_log):
    version, expected, exploratory, paths = native()
    commands = baseline.measurements(command_log, 'MESH_BOOLEAN_SETS_COMMAND',
        {k:v for k,v in expected.items() if not baseline.cancelled(k, v)})
    getters = baseline.measurements(getter_log, 'MESH_BOOLEAN_SETS_GETTER', expected)
    command_gaps, getter_gaps = [baseline.gap_names(rows, expected) for rows in [commands, getters]]
    if command_gaps != GAPS or getter_gaps != GAPS:
        raise ValueError('mesh Boolean set/Split compatibility gap set differs')
    return dict(scope='multiple mesh Boolean targets and cutters; MeshBooleanSplit getters and history',
        native_version=version, epsilon=baseline.EPSILON,
        comparison='per-object source identity, order, attributes, selection and memberships; area, oriented volume and bidirectional triangle samples after command, Undo and Redo',
        command_replayed=len(commands), getter_replayed=len(getters),
        command_matched=len(commands)-len(command_gaps), getter_matched=len(getters)-len(getter_gaps),
        command_compatibility_gaps=command_gaps, getter_compatibility_gaps=getter_gaps,
        exploratory_workflows=exploratory,
        pending=['overlapping/nested target native facet weighting',
                 'coincident/face-contact native geometry parity',
                 'curved surface meshing, output vertex channels, extreme scales, custom tolerance coefficients and performance'],
        commands=commands, getters=getters,
        sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in sorted(paths)})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command_log', type=Path)
    parser.add_argument('getter_log', type=Path)
    parser.add_argument('--output', type=Path, default=ROOT/'docs/mesh-boolean-sets-qualification.json')
    args = parser.parse_args()
    record = summarize(args.command_log.read_text(), args.getter_log.read_text())
    args.output.write_text(json.dumps(record, indent=2, allow_nan=False)+'\n')
    print('Mesh Boolean set/Split matches: commands {command_matched}/{command_replayed}, getters {getter_matched}/{getter_replayed}'.format(**record))


if __name__ == '__main__':
    main()
