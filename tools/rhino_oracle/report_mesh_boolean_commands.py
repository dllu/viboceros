"""Audit saved mesh Boolean command scenes against measured command/getter replays."""
import argparse
import base64
import gzip
import hashlib
import json
import math
from functools import lru_cache
from pathlib import Path
from .mesh_boolean_command_cases import FAMILIES, request
from .mesh_boolean_comparison import measures, sampled_boundary_distance, triangles
from .outline_comparison import sampled_boundary_distance as polyline_distance
from .qualification_sources import with_shared_sources

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
EPSILON = 1e-7
GAPS = [
    "boolean_lifecycle_difference_face_touch_pre",
    "boolean_lifecycle_difference_identical_pre",
    "boolean_lifecycle_intersection_face_touch_pre",
    "boolean_lifecycle_intersection_first_many",
    "boolean_lifecycle_intersection_identical_pre",
    "boolean_lifecycle_union_identical_normals",
    "boolean_lifecycle_union_identical_pre",
    "boolean_options_difference_auto",
    "boolean_options_gap_3_auto",
    "boolean_options_gap_4_auto",
    "boolean_options_intersection_auto",
    "boolean_tolerance_difference_document",
    "boolean_tolerance_difference_zero",
    "boolean_tolerance_gap_1_auto",
    "boolean_tolerance_gap_1_document",
    "boolean_tolerance_gap_1_zero",
    "boolean_tolerance_intersection_document",
    "boolean_tolerance_intersection_zero"
]
METADATA = ['source', 'role', 'selected', 'name', 'layer', 'color', 'color_source', 'groups']
SOURCES = with_shared_sources([
    'Cargo.toml', 'Cargo.lock', 'crates/viboceros-command/Cargo.toml',
    'crates/viboceros-command/src/lib.rs', 'crates/viboceros-command/src/object_selection.rs',
    'crates/viboceros-command/tests/mesh_boolean_command_native.rs',
    'crates/viboceros-command/tests/support/mesh_boolean.rs',
    'crates/viboceros-smlib/src/native.rs', 'crates/viboceros-smlib/native/bridge.h',
    'crates/viboceros-smlib/tests/support/mesh_boolean.rs',
    'crates/viboceros-document/src/lib.rs', 'crates/viboceros-document/src/object_geometry.rs',
    'src/app.rs', 'docs/mesh-boolean-commands.md',
] + ['tools/rhino_oracle/'+name for name in [
    'mesh_boolean_command_cases.py', 'mesh_boolean_option_cases.py', 'mesh_boolean_command_probe.py',
    'mesh_boolean_cases.py', 'generate_mesh_boolean.py', 'mesh_boolean_comparison.py',
    'report_mesh_boolean_commands.py', 'test_mesh_boolean_commands.py',
    'client.py', 'rhino_worker.py', 'structural_sdk.py', 'join_probe.py', 'merge_edges_probe.py']])


def native(families=None, counts=(85, 49), sources=None):
    rows, exploratory, versions, paths = {}, {}, set(), set(SOURCES if sources is None else sources)
    for family in (families or ['commands', 'lifecycle', 'options', 'memory', 'syntax', 'tolerance', 'policy']):
        cases = FAMILIES[family]
        name = 'mesh_boolean_'+family
        query, capture = [HERE/d/(name+'.json') for d in ['fixtures', 'observations']]
        paths.update(str(p.relative_to(ROOT)) for p in [query, capture])
        data = json.loads(capture.read_text())
        if json.loads(query.read_text()) != request(family):
            raise ValueError('mesh Boolean command request changed')
        if data['provenance']['request_sha256'] != hashlib.sha256(query.read_bytes()).hexdigest():
            raise ValueError('mesh Boolean command request digest differs')
        for sha in data['provenance']['sources'].values():
            path = HERE/'observations/source_snapshots'/(sha+'.gz')
            if hashlib.sha256(gzip.decompress(path.read_bytes())).hexdigest() != sha:
                raise ValueError('mesh Boolean command producer differs')
            paths.add(str(path.relative_to(ROOT)))
        if [r['id'] for r in data['results']] != list(cases):
            raise ValueError('mesh Boolean command native coverage differs')
        versions.add(data['engine_version'])
        for row in data['results']:
            value, identifier = row['value'], row['id']
            if value['input'] != cases[identifier]:
                raise ValueError('mesh Boolean command input differs')
            scene = HERE/'fixtures'/name/(identifier+'.3dm')
            if scene.read_bytes() != base64.b64decode(value['source_3dm']):
                raise ValueError('mesh Boolean saved source differs')
            paths.add(str(scene.relative_to(ROOT)))
            if family == 'commands' or 'Unknown command:' in value['history']:
                exploratory[identifier] = ('discovery with incomplete sets or rejected option placement'
                                          if family == 'commands' else 'native rejected exploratory option syntax')
            else:
                rows[identifier] = value
    if len(versions) != 1 or (len(rows), len(exploratory)) != counts:
        raise ValueError('mesh Boolean command capture domain differs')
    return versions.pop(), rows, exploratory, paths


@lru_cache(maxsize=512)
def compare_meshes(actual_key, expected_key):
    # Cache immutable geometry, never declarations or mutable object dictionaries.
    # Replayed/forged coordinates or faces produce a distinct comparison key.
    a, n = json.loads(actual_key), json.loads(expected_key)
    list(triangles([a]))
    if not a['faces'] or not n['faces']:
        both_empty = all(not mesh['vertices'] and not mesh['faces'] for mesh in [a, n])
        return (0., True) if both_empty else (None, False)
    error = sampled_boundary_distance([a], [n])
    area, volume = measures([a])
    native_area, native_volume = measures([n])
    return error, abs(area-native_area) <= EPSILON and abs(volume-native_volume) <= EPSILON


def mesh_key(mesh):
    return json.dumps({k:mesh[k] for k in ['vertices', 'faces']}, sort_keys=True, allow_nan=False)


def check_stage(actual, expected):
    if not isinstance(actual, list):
        raise ValueError('mesh Boolean object snapshot must be a list')
    metadata = len(actual) == len(expected) and all(
        all(a[k] == n[k] for k in METADATA) for a, n in zip(actual, expected))
    geometry, maximum = len(actual) == len(expected), 0. if len(actual) == len(expected) else None
    for a, n in zip(actual, expected):
        if 'mesh' in a and 'mesh' in n:
            error, matched = compare_meshes(mesh_key(a['mesh']), mesh_key(n['mesh']))
        elif 'polyline' in a and 'polyline' in n:
            paths=[a['polyline'],n['polyline']]
            if any(not isinstance(p,list) or len(p)<2 or any(
                    not isinstance(v,list) or len(v)!=3 or any(type(x) not in (int,float) or not math.isfinite(x) for x in v) for v in p) for p in paths):
                raise ValueError('finite intersection polylines required')
            length=lambda p:sum(math.dist(x,y) for x,y in zip(p,p[1:]))
            error=polyline_distance([paths[0]],[paths[1]])
            matched=abs(length(paths[0])-length(paths[1]))<=EPSILON
        elif not any(k in a or k in n for k in ['mesh','polyline']) and a.get('point') == n.get('point') and a.get('kind') == n.get('kind'):
            error, matched = 0., True
        else:
            error, matched = None, False
        geometry &= matched and error is not None and error <= EPSILON
        maximum = max(maximum, error) if maximum is not None and error is not None else None
    return dict(objects=actual, metadata=metadata, geometry=geometry,
                maximum_sampled_boundary_deviation=maximum)


def cancelled(identifier, value):
    return value['input'].get('cancel', False) or value['input'].get('cancel_second', False) or identifier.endswith('union_one')


def measurements(text, prefix, expected):
    rows = {}
    for line in text.splitlines():
        if prefix+' ' not in line:
            continue
        row = json.loads(line.split(prefix+' ', 1)[1])
        name = row['id']
        if name not in expected or name in rows:
            raise ValueError('unsupported or duplicate mesh Boolean command measurement')
        if row.get('error') not in [None, 'Mesh Boolean operation produced no result']:
            raise ValueError('unexpected mesh Boolean execution failure')
        if row['sources_restored_on_undo'] is not True or type(row['success']) is not bool:
            raise ValueError('mesh Boolean source ownership or status differs')
        for stage in ['after', 'undo', 'redo']:
            computed = check_stage(row[stage]['objects'], expected[name][stage])
            for field in ['metadata', 'geometry']:
                if row[stage][field] is not computed[field]:
                    raise ValueError('mesh Boolean {} declaration differs'.format(field))
            declared, distance = row[stage]['maximum_sampled_boundary_deviation'], computed['maximum_sampled_boundary_deviation']
            if ((declared is None) != (distance is None) or (distance is not None and
                    (type(declared) not in (float, int) or not math.isfinite(declared) or abs(declared-distance) > 1e-10))):
                raise ValueError('mesh Boolean sampled distance declaration differs')
        rows[name] = row
    if set(rows) != set(expected):
        raise ValueError('mesh Boolean command measurement coverage differs')
    return [rows[name] for name in sorted(rows)]


def gap_names(rows, expected):
    return sorted(r['id'] for r in rows if r['success'] != expected[r['id']]['success'] or any(
        not r[s]['metadata'] or not r[s]['geometry'] for s in ['after', 'undo', 'redo']))


def summarize(command_log, getter_log):
    version, expected, exploratory, paths = native()
    commands = measurements(command_log, 'MESH_BOOLEAN_COMMAND',
                            {k:v for k,v in expected.items() if not cancelled(k, v)})
    getters = measurements(getter_log, 'MESH_BOOLEAN_GETTER', expected)
    command_gaps, getter_gaps = gap_names(commands, expected), gap_names(getters, expected)
    if command_gaps != GAPS or getter_gaps != GAPS:
        raise ValueError('mesh Boolean command or getter compatibility gap set differs')
    return dict(scope='mesh Boolean commands and application getters on frozen independent scenes',
                native_version=version, epsilon=EPSILON,
                comparison='per-object order, source identity, selection, name, layer, color, groups, area, oriented volume and bidirectional triangle surface samples after command, Undo and Redo',
                command_replayed=len(commands), getter_replayed=len(getters),
                command_matched=len(commands)-len(command_gaps), getter_matched=len(getters)-len(getter_gaps),
                command_compatibility_gaps=command_gaps, getter_compatibility_gaps=getter_gaps,
                exploratory_workflows=exploratory,
                pending=['custom mesh-intersection coefficient settings beyond the captured 0.0001 default',
                         'coincident, face-contact and thin-overlap native geometry parity',
                         'stored output normal/texture/color channels, curved surface meshing, extremes and performance'],
                commands=commands, getters=getters,
                sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in sorted(paths)})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command_log', type=Path)
    parser.add_argument('getter_log', type=Path)
    parser.add_argument('--output', type=Path, default=ROOT/'docs/mesh-boolean-command-qualification.json')
    args = parser.parse_args()
    result = summarize(args.command_log.read_text(), args.getter_log.read_text())
    args.output.write_text(json.dumps(result, indent=2, allow_nan=False)+'\n')
    print('Mesh Boolean matches: commands {command_matched}/{command_replayed}, getters {getter_matched}/{getter_replayed}'.format(**result))


if __name__ == '__main__':
    main()
