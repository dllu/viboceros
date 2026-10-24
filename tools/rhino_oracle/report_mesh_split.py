"""Audit command partitions, logical face boundaries, selection, and history."""
import argparse
import hashlib
import json
import math
from pathlib import Path
from . import report_mesh_boolean_commands as baseline
from .qualification_sources import with_shared_sources
from .report_mesh_split_kernel import SOURCES as KERNEL_SOURCES, validate_ngons

ROOT, HERE = baseline.ROOT, baseline.HERE
GAPS = [
    'mesh_split_controls_face_contact_drop',
    'mesh_split_controls_mixed_cutter',
    'mesh_split_controls_three_cutters',
    'mesh_split_controls_two_cutters',
    'mesh_split_policy_brep_target_post',
    'mesh_split_policy_brep_target_pre',
    'mesh_split_policy_plane_three_point',
    'mesh_split_policy_plane_through_point',
    'mesh_split_policy_plane_world_yz',
    'mesh_split_selection_cross_both',
    'mesh_split_selection_curve_xz',
    'mesh_split_two_cutters',
]
SOURCES = with_shared_sources(baseline.SOURCES + KERNEL_SOURCES + [
    'crates/viboceros-command/tests/mesh_split_native.rs',
    'crates/viboceros-command/tests/support/mesh_split.rs',
    'docs/commands/mesh-split.md',
    'tools/rhino_oracle/report_mesh_split.py',
    'tools/rhino_oracle/test_mesh_split.py',
])


def native():
    from .mesh_split_evidence import native as captured
    return captured(['discovery', 'controls', 'policy', 'selection'], SOURCES)


def cancelled(value):
    case = value['input']
    return (case.get('cancel', False) or case.get('cancel_second', False) or
            'menu' in case or case.get('cross_inputs', False) and not case['second'])


def ngon_objects(mesh, ngon):
    vertices = [mesh['vertices'][i] for i in ngon['vertices']]
    return [dict(mesh=dict(vertices=mesh['vertices'], faces=[mesh['faces'][i] for i in ngon['faces']])),
            dict(polyline=vertices + vertices[:1])]


def same_ngons(a, b):
    validate_ngons(a)
    validate_ngons(b)
    if len(a['ngons']) != len(b['ngons']):
        return False
    used = set()
    for ngon in a['ngons']:
        found = None
        for i, other in enumerate(b['ngons']):
            if i in used:
                continue
            first, second = ngon_objects(a, ngon), ngon_objects(b, other)
            # Supply neutral attributes; the parent objects are checked separately.
            first = [dict.fromkeys(baseline.METADATA) | obj for obj in first]
            second = [dict.fromkeys(baseline.METADATA) | obj for obj in second]
            if baseline.check_stage(first, second)['geometry']:
                found = i
                break
        if found is None:
            return False
        used.add(found)
    return True


def check_stage(actual, expected):
    result = baseline.check_stage(actual, expected)
    result['ngons'] = len(actual) == len(expected) and all(
        'ngons' not in n.get('mesh', {}) or
        'ngons' in a.get('mesh', {}) and same_ngons(a['mesh'], n['mesh'])
        for a, n in zip(actual, expected))
    return result


def measurements(log, prefix, expected):
    rows = {}
    for line in log.splitlines():
        if prefix + ' ' not in line:
            continue
        row = json.loads(line.split(prefix + ' ', 1)[1])
        identifier = row['id']
        if identifier not in expected or identifier in rows:
            raise ValueError('unsupported or duplicate mesh split measurement')
        if row['error'] is not None or row['sources_restored_on_undo'] is not True or type(row['success']) is not bool:
            raise ValueError('mesh split ownership or status differs')
        for stage in ['after', 'undo', 'redo']:
            computed = check_stage(row[stage]['objects'], expected[identifier][stage])
            if any(row[stage][field] is not computed[field] for field in ['geometry', 'metadata', 'ngons']):
                raise ValueError('mesh split geometry, metadata or n-gon declaration differs')
            declared = row[stage]['maximum_sampled_boundary_deviation']
            distance = computed['maximum_sampled_boundary_deviation']
            if ((declared is None) != (distance is None) or distance is not None and
                    (type(declared) not in (int, float) or not math.isfinite(declared) or abs(declared-distance) > 1e-10)):
                raise ValueError('mesh split sampled distance declaration differs')
        rows[identifier] = row
    if set(rows) != set(expected):
        raise ValueError('mesh split measurement coverage differs')
    return [rows[k] for k in sorted(rows)]


def summarize(command_log, getter_log):
    version, expected, paths = native()
    commands = measurements(command_log, 'MESH_SPLIT_COMMAND',
                            {k: v for k, v in expected.items() if not cancelled(v)})
    getters = measurements(getter_log, 'MESH_SPLIT_GETTER', expected)
    def gaps(rows):
        return sorted(r['id'] for r in rows if r['success'] != expected[r['id']]['success'] or any(
            not r[s][k] for s in ['after', 'undo', 'redo'] for k in ['metadata', 'geometry', 'ngons']))
    if any(gaps(rows) != GAPS for rows in [commands, getters]):
        raise ValueError('mesh split compatibility gaps changed')
    return dict(scope='MeshSplit command and picker, shared options, projected curves, infinite planes and history',
                native_version=version, epsilon=baseline.EPSILON,
                comparison='Ordered objects, source identity, attributes, groups, selection, area, volume, bidirectional surface samples, and unordered n-gon boundary lengths/coverage after execution, Undo and Redo',
                command_replayed=len(commands), getter_replayed=len(getters),
                command_matched=len(commands)-len(GAPS), getter_matched=len(getters)-len(GAPS),
                command_compatibility_gaps=GAPS, getter_compatibility_gaps=GAPS,
                pending=['native component order with multiple cutters, B-reps and shared input sets',
                         'mixed coplanar grouping and three-cutter surface truncation',
                         'native n-gon boundaries on infinite planes and projected curves, contact face grouping',
                         'curved meshing, extreme scales, near contacts and comparative performance; vertex channels have a separate qualification'],
                commands=commands, getters=getters,
                sha256={p: hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in sorted(paths)})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command_log', type=Path)
    parser.add_argument('getter_log', type=Path)
    parser.add_argument('--output', type=Path, default=ROOT/'docs/mesh-split-qualification.json')
    args = parser.parse_args()
    record = summarize(args.command_log.read_text(), args.getter_log.read_text())
    args.output.write_text(json.dumps(record, indent=2, allow_nan=False) + '\n')
    print('MeshSplit: {command_matched}/{command_replayed} commands, {getter_matched}/{getter_replayed} getters'.format(**record))


if __name__ == '__main__':
    main()
