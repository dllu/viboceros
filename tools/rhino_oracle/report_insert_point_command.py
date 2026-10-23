"""Qualify InsertPoint staging and retain explicit mesh representation differences."""
import argparse
from collections import Counter
import hashlib
import json
import math
import re
from pathlib import Path
from .qualification_sources import with_shared_sources
from .report_insert_point import FAMILIES as KERNEL_FAMILIES, SOURCES as KERNEL_SOURCES
from .report_subd_box import HERE, ROOT, error

FAMILIES = dict(KERNEL_FAMILIES, insert_point_holdouts_v2=12, insert_point_ngons=12,
                insert_point_mesh_sdk=16, insert_point_mesh_sdk_origins=16)
CELL_GAPS = {'insert_point_full_mesh_adjacent', 'insert_point_full_mesh_same_edge',
             'insert_point_full_mesh_strip_mid'}
HOLDOUT_CELL_GAPS = {'insert_point_holdout_mesh_ngon_island',
                     'insert_point_holdout_mesh_reverse'}
NGON_CELL_GAPS = {'insert_point_ngon_two_overlays_shared'}
SOURCES = with_shared_sources(KERNEL_SOURCES + [
    'crates/viboceros-geometry/src/mesh.rs',
    'crates/viboceros-command/tests/insert_point_command.rs',
    'crates/viboceros-command/tests/insert_point_staging.rs',
    'crates/viboceros-command/tests/insert_point_ngons.rs',
    'crates/viboceros-command/tests/support/insert_point.rs',
    'crates/viboceros-geometry/tests/support/subdivision_native.rs',
    'crates/viboceros-document/src/lib.rs',
    'crates/viboceros-document/src/history.rs',
    'src/app.rs', 'src/app/crease.rs', 'src/app/crease/tests.rs',
    'src/viewport.rs', 'src/viewport/edge_point.rs', 'src/viewport/edge_point/tests.rs',
    'src/viewport/selection.rs', 'src/viewport/selection_clipping.rs',
] + ['tools/rhino_oracle/' + name for name in [
    'insert_point_holdout_cases.py', 'insert_point_mesh_sdk_cases.py',
    'insert_point_mesh_sdk_probe.py', 'generate_insert_point_mesh_sdk.py',
    'report_insert_point_command.py', 'test_insert_point_command.py',
    'fixtures/insert_point_holdouts.json',
    'observations/insert_point_holdouts_import_failure.json',
    'observations/insert_point_holdouts_import_failure.log',
]])


def capture(family):
    query = json.loads((HERE/'fixtures'/(family+'.json')).read_text())
    native = json.loads((HERE/'observations'/(family+'.json')).read_text())
    ids = [r['id'] for r in native['results']]
    if (len(ids) != FAMILIES[family] or len(set(ids)) != len(ids)
            or ids != [r['id'] for r in query['operations']]):
        raise ValueError('missing, duplicate or reordered insertion observations')
    return native


def witnessed(family):
    result = {}
    for row in capture(family)['results']:
        value = row.get('value')
        if value is None or 'Unknown command' in value['history']:
            raise ValueError('missing native insertion workflow')
        traces = value['input_trace']
        if len(traces) != 1:
            raise ValueError('unexpected insertion input trace')
        trace = traces[0]
        actions = value['input']['actions']
        clicks = trace.get('clicks', [])
        wanted = [i for i, a in enumerate(actions) if isinstance(a, dict)]
        if (trace.get('errors') or trace.get('completed') != len(actions)
                or trace.get('expected') != len(actions)
                or [c['action'] for c in clicks] != wanted):
            raise ValueError('incomplete native physical insertion input')
        for click in clicks:
            if (click.get('click_acknowledged') is not True
                    or click.get('motion_acknowledged') is not True
                    or len(click.get('ray', [])) != 2
                    or any(len(p) != 3 for p in click['ray'])
                    or len(click.get('projected_point', [])) != 3
                    or any(type(x) not in (int, float) or not math.isfinite(x)
                           for p in click['ray']+[click['projected_point']] for x in p)):
                raise ValueError('missing native physical point witness')
        result[row['id']] = value
    return result


def cycle(values):
    return min(tuple(values[i:]+values[:i]) for i in range(len(values)))


def cells(mesh):
    return sorted(cycle([tuple(mesh['vertices'][i]) for i in f]) for f in mesh['faces'])


def sdk_calibration():
    """Compare public APIs only; their inputs are captured *outputs*, not picks."""
    from .insert_point_mesh_sdk_cases import CASES
    result = []
    base = capture('insert_point_mesh_sdk')['results']
    origins = capture('insert_point_mesh_sdk_origins')['results']
    for row, rotated in zip(base, origins):
        recipe = CASES[row['id']]
        if row['value']['input'] != recipe or rotated['value']['input'] != recipe:
            raise ValueError('public meshing recipe differs')
        source = next(r['value'] for r in capture(recipe['capture'])['results']
                      if r['id'] == recipe['source_id'])
        native = next(o['mesh'] for o in source['after'] if o['source'] == 0)
        target = dict(vertices=recipe['vertices'], faces=recipe['captured_faces'])
        if Counter(cells(target)) - Counter(cells(native)):
            raise ValueError('calibration region is not from the captured mesh')
        output = row['value']['results']
        matches = {name: cells(value.get('mesh', value)) == cells(target)
                   for name, value in output.items()}
        rotations = rotated['value']['origins']
        if set(rotations) != {str(i) for i in range(len(recipe['vertices']))}:
            raise ValueError('missing public meshing boundary origin')
        result.append(dict(id=row['id'], api_matches=matches,
                           matching_origins=[i for i in range(len(recipe['vertices']))
                                             if cells(rotations[str(i)]['quads']) == cells(target)]))
    return result


def source_hashes():
    paths = list(SOURCES)
    for family in FAMILIES:
        paths.extend('tools/rhino_oracle/'+folder+'/'+family+'.json'
                     for folder in ['fixtures', 'observations'])
        paths.extend(str(p.relative_to(ROOT)) for p in sorted((HERE/'fixtures'/family).glob('*.3dm')))
    return {p: hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in dict.fromkeys(paths)}


def replay_rows(text, stage, expected):
    rows = [json.loads(s) for s in re.findall(r'^INSERT_POINT_'+stage+r' (.+)$', text, re.M)]
    if len(rows) != len(expected) or {r['id'] for r in rows} != set(expected):
        raise ValueError('missing, duplicate or unrelated insertion '+stage+' replay')
    return sorted(rows, key=lambda r: list(expected).index(r['id']))


def check_mesh(actual, native, key, group):
    pieces = key not in {'primary': CELL_GAPS, 'holdout': HOLDOUT_CELL_GAPS, 'ngon': NGON_CELL_GAPS}[group]
    facets = key != 'insert_point_holdout_mesh_reverse' if group == 'holdout' else pieces
    for field, expected in [('qualified', True), ('logical_regions', True),
                            ('ngon_overlays_correspond', True),
                            ('channels', True), ('stored_pieces_correspond', pieces),
                            ('stored_facets_correspond', facets)]:
        if actual.get(field) is not expected:
            raise ValueError('unexpected insertion mesh '+field+' result: '+key)
    for field, values in [('vertices', native['vertices']), ('faces', native['faces']),
                           ('ngons', native['ngons'])]:
        if type(actual.get(field)) is not int or actual[field] != len(values):
            raise ValueError('insertion mesh count differs')
    wanted = sorted([list(cycle(f)) for f in native['faces']])
    if actual.get('native_cells') != wanted:
        raise ValueError('insertion native cells differ')
    records = actual.get('actual_cells', [])
    if len(records) != len(native['faces']) or any(
            len(f) not in (3, 4) or len(set(f)) != len(f)
            or any(type(i) is not int or not 0 <= i < len(native['vertices']) for i in f)
            for f in records):
        raise ValueError('invalid insertion cell diagnostic')
    if (records == wanted) is not pieces:
        raise ValueError('insertion cell gap relabeled')
    return error(actual.get('maximum_error'))


def summarize(text):
    expected = witnessed('insert_point_full')
    holdouts = witnessed('insert_point_holdouts_v2')
    ngon_expected = witnessed('insert_point_ngons')
    commands = replay_rows(text, 'COMMAND', expected)
    getters = replay_rows(text, 'GETTER', expected)
    extra = replay_rows(text, 'HOLDOUT', holdouts)
    ngons = replay_rows(text, 'NGON', ngon_expected)
    ngon_getters = replay_rows(text, 'NGON_GETTER', ngon_expected)
    maximum = 0.
    for stage, rows, wanted in [('command', commands, expected), ('getter', getters, expected),
                                 ('holdout', extra, holdouts), ('ngon_command', ngons, ngon_expected),
                                 ('ngon_getter', ngon_getters, ngon_expected)]:
        for row in rows:
            value = wanted[row['id']]
            target = next(o for o in value['after'] if o['source'] == 0)
            for flag in ['identity', 'metadata'] + ([] if stage == 'holdout' else
                         ['history', 'preview'] + (['selection'] if stage.endswith('getter') else [])):
                if row.get(flag) is not True:
                    raise ValueError('unqualified insertion '+flag)
            if stage != 'holdout':
                changed = any(o['name'] == 'Undo witness' for o in value['undo'])
                if row.get('changed') is not changed:
                    raise ValueError('incorrect insertion history admission')
            actual = row['comparison']
            if 'mesh' in target:
                group = 'ngon' if stage.startswith('ngon') else 'holdout' if stage == 'holdout' else 'primary'
                maximum = max(maximum, check_mesh(actual, target['mesh'], row['id'], group))
            else:
                native = target['subd']
                if actual.get('qualified') is not True or native['valid'] is not True:
                    raise ValueError('unqualified insertion SubD')
                for field, values in [('vertices', native['points']), ('faces', native['faces'])]:
                    if type(actual.get(field)) is not int or actual[field] != len(values):
                        raise ValueError('insertion SubD count differs')
                if stage == 'holdout':
                    if (actual.get('surface_sampled') is not True
                            or type(actual.get('edges')) is not int
                            or actual['edges'] != len(native['edges'])):
                        raise ValueError('unqualified insertion SubD samples')
                    maximum = max(maximum, error(actual.get('maximum_control_or_limit_point_error')),
                                  error(actual.get('maximum_sampled_surface_error')))
                else:
                    for flag in ['logical_regions', 'channels', 'stored_pieces_correspond', 'stored_facets_correspond']:
                        if actual.get(flag) is not True:
                            raise ValueError('unqualified insertion SubD '+flag)
                    maximum = max(maximum, error(actual.get('maximum_error')))
    engines = {capture(f)['engine_version'] for f in FAMILIES}
    if len(engines) != 1:
        raise ValueError('mixed insertion native versions')
    return dict(native_engine_version=engines.pop(), model_epsilon=1e-6,
                command_implemented=True, command_cases=len(commands), getter_cases=len(getters),
                additional_geometry_cases=len(extra), captured_command_workflows=69,
                ngon_cases=len(ngons), ngon_getter_cases=len(ngon_getters),
                calibration_recipes=32, failed_import_requested_workflows=12,
                failed_import_qualified_workflows=0, maximum_error=maximum,
                primary_cell_gaps=sorted(CELL_GAPS), holdout_cell_gaps=sorted(HOLDOUT_CELL_GAPS),
                holdout_region_gaps=[], ngon_cell_gaps=sorted(NGON_CELL_GAPS),
                cases=commands, getters=getters, holdouts=extra, ngons=ngons,
                ngon_getters=ngon_getters, sdk_calibration=sdk_calibration(),
                full_native_parity=False, performance_qualified=False,
                limits=[
                    'Thirty-three command/staging and thirty-three application getter replays use saved source archives and independent physical Top-view input witnesses. Identity, attributes, groups, selection, preview isolation, document Undo/Redo, local Undo and cancellation match these captures.',
                    'Primary mesh logical regions, vertex channels and sampled geometry agree, but three planar cases retain different stored triangle/quad cells and fixed 0-2 facets. Eighteen of twenty-one primary workflows also match those stored representations.',
                    'Twelve additional spatial, triangular, original-edge, n-gon and raw-seam cases preserve control/vertex coordinates, explicit n-gon overlays, logical regions, channels and sampled geometry. Both earlier n-gon grouping differences are resolved. Two cases still have different stored cells, including one different fixed-facet layout.',
                    'Twelve new n-gon command and getter cases cover uncovered triangles, single-face overlays, repeated points on one edge, reordered source faces, triangulated fans, interior picks, paths across fans and adjacent n-gons. Original n-gon adjacency survives through split original edges; new tessellation edges only join automatic n-gons for regions with more than four boundary vertices. All explicit overlays and logical regions agree. One case with adjacent single-quad overlays still has different stored cells and fixed facets.',
                    'The public meshing API calibration uses captured output regions, never independent input witnesses. Neither public triangulation nor quad conversion reproduces all native InsertPoint cell choices, even across every boundary origin. It does not qualify command or kernel parity.',
                    'Original source edges remain the pick cage, including after new chords appear. Coordinates choose the nearest spatial source edge; viewport picks use screen distance and a constrained spatial edge parameter. Parallel crossing projection pins the first supplied view normal; perspective crossing paths and changing views are not natively qualified.',
                    'Accepted points are staged until Enter. Cancel, Escape and starting another command discard them. Empty or Undo-all acceptance creates no geometry history and preserves redo. Stale source geometry, tolerance or editability rejects admission.',
                    'The path budget is 256 picks and one million source point-plus-edge components, subject to native SubD archive limits. Intersecting inserted chords, arbitrary non-manifold/spatial topology, constrained Osnap behavior, native grip-state behavior, continuous surface error and comparative performance remain unqualified.',
                    'Earlier discovery, unwitnessed physical captures and the incomplete regions getter remain in the separate kernel record. The first holdout attempt failed during worker import; its request, host log and frozen sources are retained, but no native response survived. No workflows from that failed attempt are qualified.',
                ], sha256=source_hashes())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('logs', nargs='+', type=Path)
    args = parser.parse_args()
    report = summarize('\n'.join(p.read_text() for p in args.logs))
    (ROOT/'docs/insert-point-command-qualification.json').write_text(
        json.dumps(report, indent=2, allow_nan=False)+'\n')


if __name__ == '__main__':
    main()
