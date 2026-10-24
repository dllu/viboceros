"""Physical getter, ray, preselection, and independent native partition checks."""
import argparse
import hashlib
import json
import math
from pathlib import Path
from . import report_mesh_trim as trim
from .mesh_boolean_comparison import measures, sampled_boundary_distance
from .qualification_sources import with_shared_sources
from .report_mesh_split_kernel import validate_ngons

ROOT = trim.ROOT
FAMILIES = ['curve_controls', 'pick_controls', 'settled_picks', 'direct_getter',
            'getter_controls', 'verified_getters', 'plane_getters']
REPLAYED = ['getter_controls', 'verified_getters', 'plane_getters']
GAPS = ['mesh_trim_getter_controls_two_meshes_sdk', 'mesh_trim_plane_getters_three_point',
        'mesh_trim_plane_getters_world_yz']
SOURCES = with_shared_sources(trim.SOURCES + [
    'tools/rhino_oracle/report_mesh_trim_getters.py',
    'tools/rhino_oracle/test_mesh_trim_getters.py',
    'docs/mesh-trim-getters.md',
])


def verify_trace(value):
    trace = value.get('trim_pick_trace')
    if trace is None:
        return
    expected, actual = trace['expected'], trace['actual']
    if len(expected) != len(value['input']['trim_picks']) or len(actual) != len(expected):
        raise ValueError('native trim physical click coverage differs')
    for index, (e, a) in enumerate(zip(expected, actual)):
        if (e['pixel'] != a['pixel'] or e['viewport'] != a['viewport'] or
                a['prompt'] != 'Select object to trim'):
            raise ValueError('native trim physical click missed its getter or viewport')
        coordinates = e['ray'] + [e['point']] + [h['point'] for h in e['hits'] if h['point'] is not None]
        if not all(len(p) == 3 and all(type(x) in (int, float) and math.isfinite(x) for x in p) for p in coordinates):
            raise ValueError('native trim pick query is not finite')
        start, end = e['ray']
        direction = [b-a for a, b in zip(start, end)]
        length = sum(x*x for x in direction)
        if length == 0. or e['point'] != value['input']['trim_picks'][index]:
            raise ValueError('native trim ray or requested point differs')
        def distance(point):
            delta = [b-a for a, b in zip(start, point)]
            parameter = sum(a*b for a, b in zip(delta, direction))/length
            return math.sqrt(sum((a-parameter*b)**2 for a, b in zip(delta, direction)))
        if distance(e['point']) > .1 or any(distance(h['point']) > 1e-7 for h in e['hits'] if h['point'] is not None):
            raise ValueError('native trim pick points missed the recorded ray')
        wanted = value['input'].get('trim_targets', [0]*len(expected))[index]
        if e['hits'] and not any(h['source'] == wanted and h['point'] is not None for h in e['hits']):
            raise ValueError('native shaded query missed the intended source')


def verify_sdk(value):
    rows = value.get('trim_split_control')
    if rows is None:
        return None
    if len(rows) != 1 or rows[0]['inputs_unchanged'] is not True:
        raise ValueError('native private split changed its inputs')
    parts = rows[0]['parts']
    for part in parts:
        validate_ngons(part)
    target = next(o['mesh'] for o in value['before'] if o['source'] == 2)
    areas = sorted(measures([part])[0] for part in parts)
    wanted = [2.] * 4 if len(value['input']['sdk_cutters']) == 2 else [4.] * 2
    if len(areas) != len(wanted) or any(abs(a-b) > 1e-7 for a, b in zip(areas, wanted)):
        raise ValueError('native SDK rectangle partition differs')
    deviation = sampled_boundary_distance(parts, [target])
    if deviation > 1e-7:
        raise ValueError('native SDK partition changed the input surface')
    after = next(o['mesh'] for o in value['after'] if o['name'] == 'Source 2')
    return dict(part_areas=areas, union_area=measures(parts)[0],
                maximum_sampled_boundary_deviation=deviation, command_retained_area=measures([after])[0],
                command_surface_deviation=sampled_boundary_distance([after], [target]))


def native():
    version, rows, paths = trim.native(FAMILIES)
    for value in rows.values():
        verify_trace(value)
        verify_sdk(value)
    return version, rows, paths | set(SOURCES)


def measurements(log, suffix, expected):
    rows = []
    for family in REPLAYED:
        rows += trim.baseline.measurements(log, 'MESH_TRIM_'+family.upper()+'_'+suffix,
                   {k: v for k, v in expected.items() if k.startswith('mesh_trim_'+family+'_')})
    return sorted(rows, key=lambda row: row['id'])


def summarize(command_log, getter_log):
    version, captured, paths = native()
    expected = {k: v for k, v in captured.items() if any(k.startswith('mesh_trim_'+f+'_') for f in REPLAYED)}
    commands = measurements(command_log, 'COMMAND', expected)
    getters = measurements(getter_log, 'GETTER', expected)
    def gaps(rows):
        return sorted(r['id'] for r in rows if r['success'] != expected[r['id']]['success'] or any(
            not r[s][k] for s in ['after', 'undo', 'redo'] for k in ['geometry', 'metadata', 'ngons']) or
            any(not r[s]['channels']['matched'] for s in ['after', 'undo', 'redo']))
    if any(gaps(rows) != GAPS for rows in [commands, getters]):
        raise ValueError('mesh trim getter compatibility gaps changed')
    sdk = {k: verify_sdk(v) for k, v in captured.items() if 'trim_split_control' in v}
    return dict(scope='MeshTrim physical preselection and plane getters, delivered pixel/ray checks and independent SDK partitions',
                native_version=version, epsilon=1e-7, captured=len(captured),
                diagnostic_capture_ids=sorted(set(captured)-set(expected)), sdk_controls=sdk,
                command_replayed=len(commands), getter_replayed=len(getters),
                command_matched=len(commands)-len(GAPS), getter_matched=len(getters)-len(GAPS),
                command_compatibility_gaps=GAPS, getter_compatibility_gaps=GAPS,
                comparison='Ordered identity, metadata, selection, surfaces, coherent vertex-channel samples, source storage, Undo/Redo; SDK rectangle areas and bidirectional union surface samples; physical clicks match intended viewport pixels and getter prompts',
                pending=['The native command can retain all four SDK regions for a vertical target; the underlying cause remains unresolved',
                         'Native temporary-plane getters leave the target unchanged in both direct physical controls',
                         'Curved meshing, extreme scales, near contacts and comparative performance'],
                commands=commands, getters=getters,
                sha256={p: hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in sorted(paths)})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command_log', type=Path)
    parser.add_argument('getter_log', type=Path)
    parser.add_argument('--output', type=Path, default=ROOT/'docs/mesh-trim-getter-qualification.json')
    args = parser.parse_args()
    record = summarize(args.command_log.read_text(), args.getter_log.read_text())
    args.output.write_text(json.dumps(record, indent=2, allow_nan=False)+'\n')
    print('MeshTrim getters: {command_matched}/{command_replayed} commands, {getter_matched}/{getter_replayed} getters'.format(**record))


if __name__ == '__main__':
    main()
