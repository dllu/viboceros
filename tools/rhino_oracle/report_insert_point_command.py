"""Qualify InsertPoint staging and retain explicit mesh representation differences."""
import argparse
import hashlib
import json
import math
import re
from pathlib import Path
from .qualification_sources import with_shared_sources
from . import report_insert_point_calibration as calibration
from . import report_insert_point_inputs as inputs
from .report_insert_point_calibration import cycle
from .report_insert_point import FAMILIES as KERNEL_FAMILIES, SOURCES as KERNEL_SOURCES
from .report_subd_box import HERE, ROOT, error

FAMILIES = dict(KERNEL_FAMILIES, insert_point_holdouts_v2=12, insert_point_ngons=12,
                insert_point_mesh_sdk=16, insert_point_mesh_sdk_origins=16,
                insert_point_mesh_sdk_methods=16, insert_point_mesh_sdk_spatial=12, insert_point_mesh_shapes=12,
                insert_point_pick_depth=12, insert_point_pick_depth_v2=8, insert_point_spatial_layouts=12)
CELL_GAPS = {'insert_point_full_mesh_same_edge'}
DEPTH_CELL_GAPS = set()
SPATIAL_CELL_GAPS = {'insert_point_spatial_bottom_mid_left_high', 'insert_point_spatial_tilted_plane',
                     'insert_point_spatial_same_edge_warp'}
FACET_GAPS = CELL_GAPS | SPATIAL_CELL_GAPS
SURFACE_GAPS = {'insert_point_spatial_bottom_mid_left_high', 'insert_point_spatial_same_edge_warp'}
HOLDOUT_CELL_GAPS = NGON_CELL_GAPS = set()
REPLAYS = [
    ('COMMAND', 'cases', 'insert_point_full'), ('GETTER', 'getters', 'insert_point_full'),
    ('HOLDOUT', 'holdouts', 'insert_point_holdouts_v2'),
    ('NGON', 'ngons', 'insert_point_ngons'), ('NGON_GETTER', 'ngon_getters', 'insert_point_ngons'),
    ('SHAPE', 'shapes', 'insert_point_mesh_shapes'),
    ('SHAPE_GETTER', 'shape_getters', 'insert_point_mesh_shapes'),
    ('DEPTH', 'depths', 'insert_point_pick_depth'),
    ('DEPTH_GETTER', 'depth_getters', 'insert_point_pick_depth'),
    ('DEPTH_V2', 'depth_witnesses', 'insert_point_pick_depth_v2'),
    ('DEPTH_V2_GETTER', 'depth_witness_getters', 'insert_point_pick_depth_v2'),
    ('SPATIAL', 'spatial', 'insert_point_spatial_layouts'),
    ('SPATIAL_GETTER', 'spatial_getters', 'insert_point_spatial_layouts'),
]
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
    'src/viewport/picking.rs', 'src/viewport/camera.rs',
    'crates/viboceros-drafting/src/plane.rs',
    'src/viewport/selection.rs', 'src/viewport/selection_clipping.rs',
] + ['tools/rhino_oracle/' + name for name in [
    'insert_point_holdout_cases.py', 'insert_point_mesh_sdk_cases.py',
    'insert_point_mesh_sdk_probe.py', 'generate_insert_point_mesh_sdk.py',
    'insert_point_mesh_methods.py', 'insert_point_spatial_sdk.py', 'insert_point_spatial_sdk_cases.py', 'report_insert_point_calibration.py',
    'report_insert_point_inputs.py',
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
    return inputs.witnessed(capture, family)


def sdk_calibration():
    return calibration.sdk_calibration(capture)


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


def surface_error(value, qualified):
    if qualified:
        return error(value)
    if type(value) not in (int, float) or not math.isfinite(value) or value <= 1e-6:
        raise ValueError('invalid unqualified surface error')
    return value


def check_mesh(actual, native, key):
    pieces = key not in CELL_GAPS | DEPTH_CELL_GAPS | SPATIAL_CELL_GAPS
    facets = key not in FACET_GAPS
    surface_qualified = key not in SURFACE_GAPS
    fixed_error = surface_error(actual.get('maximum_fixed_facet_sample_error'), surface_qualified)
    short_error = surface_error(actual.get('maximum_error'), surface_qualified)
    for field, expected in [('qualified', surface_qualified), ('logical_regions', True),
                            ('fixed_facet_surface_qualified', surface_qualified),
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
    return short_error, fixed_error


def summarize(text):
    witnesses = {family: witnessed(family) for family in dict.fromkeys(f for _, _, f in REPLAYS)}
    stages = [(stage.lower(), replay_rows(text, stage, witnesses[family]), witnesses[family])
              for stage, _, family in REPLAYS]
    replays = {key: rows for (_, key, _), (_, rows, _) in zip(REPLAYS, stages)}
    maximum = maximum_fixed = maximum_qualified = 0.
    for stage, rows, wanted in stages:
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
                geometry_error, fixed_error = check_mesh(actual, target['mesh'], row['id'])
                maximum = max(maximum, geometry_error)
                maximum_fixed = max(maximum_fixed, fixed_error)
                if actual['qualified']:
                    maximum_qualified = max(maximum_qualified, geometry_error, fixed_error)
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
                maximum_qualified = max(maximum_qualified,
                    error(actual.get('maximum_error', actual.get('maximum_control_or_limit_point_error'))),
                    error(actual.get('maximum_sampled_surface_error', 0.)))
    engines = {capture(f)['engine_version'] for f in FAMILIES}
    if len(engines) != 1:
        raise ValueError('mixed insertion native versions')
    return dict(native_engine_version=engines.pop(), model_epsilon=1e-6,
                command_implemented=True,
                command_cases=sum(len(rows) for stage, rows, _ in stages if stage != 'holdout' and not stage.endswith('getter')),
                getter_cases=sum(len(rows) for stage, rows, _ in stages if stage.endswith('getter')),
                primary_cases=len(replays['cases']), additional_geometry_cases=len(replays['holdouts']),
                captured_command_workflows=sum(FAMILIES[f] for f in FAMILIES if 'sdk' not in f),
                ngon_cases=len(replays['ngons']), ngon_getter_cases=len(replays['ngon_getters']),
                shape_cases=len(replays['shapes']), depth_cases=len(replays['depths'])+len(replays['depth_witnesses']),
                spatial_cases=len(replays['spatial']),
                calibration_recipes=60, failed_import_requested_workflows=12,
                failed_import_qualified_workflows=0, maximum_error=max(maximum, maximum_fixed),
                maximum_control_or_shortest_diagonal_sample_error=maximum,
                maximum_fixed_facet_sample_error=maximum_fixed, maximum_qualified_error=maximum_qualified,
                geometry_unqualified=sorted(SURFACE_GAPS),
                primary_cell_gaps=sorted(CELL_GAPS), holdout_cell_gaps=[], holdout_region_gaps=[],
                ngon_cell_gaps=[], shape_cell_gaps=[], depth_cell_gaps=sorted(DEPTH_CELL_GAPS),
                facet_gaps=sorted(FACET_GAPS), spatial_cell_gaps=sorted(SPATIAL_CELL_GAPS), **replays,
                sdk_calibration=sdk_calibration(), methods_calibration=calibration.methods_calibration(capture),
                spatial_sdk_calibration=calibration.spatial_calibration(capture),
                full_native_parity=False, performance_qualified=False,
                limits=[
                    'Seventy-seven command/staging and seventy-seven application getter replays use saved source archives and independent physical Top-view input witnesses. Identity, attributes, groups, selection, preview isolation, document Undo/Redo, local Undo and cancellation match these captures.',
                    'All original sixty-five command and getter workflows now agree on coordinates and both surface-sampling policies. The two earlier warped depth surface failures and all three depth cell gaps are resolved by coplanarity-aware pairing and full split-boundary origins. One planar primary repeated-edge cell/facet difference remains.',
                    'Twelve new independent spatial scenes cover changed warp, aspect, winding, cyclic origin, adjacent picks, reversed paths and repeated same-edge points. Ten match geometry at the unchanged epsilon; one of these planar scenes differs in stored cells. The upper-left second pick and repeated-edge warped cases still differ in geometry under both sampling policies and are explicitly unqualified. The repeated-edge scene now completes instead of rejecting a nearly collinear quad.',
                    'Twelve geometry holdouts cover spatial, triangular, original-edge, n-gon and raw-seam cases. Twelve n-gon command/getter cases cover overlays, repeated boundary picks, reordered faces, fans and paths across adjacent n-gons. All now match stored cells and facets.',
                    'Twelve independent shape scenes cover skewed, trapezoid, rhombus, tall, rotated, reversed and warped quad boundaries. All match stored cells and facets.',
                    'Twenty independent warped Mesh/SubD depth scenes cover inside/outside surface rays, translated sources, shifted camera targets, second picks and local Undo. Picks use the original control display mesh with fixed 0-2 facets, falling back to the camera-target plane, then closest 3D edge parameter. Generic SDK MeshRay hits use a different quad diagonal and are retained only as triangulation-policy diagnostics.',
                    'The sixty public meshing API calibration recipes use captured output regions, never independent input witnesses. No tested public triangulation or quad conversion reproduces all native InsertPoint cells, including relaxed 3D conversion across every boundary origin in the twelve spatial regions. Calibration does not qualify command or kernel parity.',
                    'Original source edges remain the pick cage after new chords appear. Coordinates choose the nearest spatial source edge; viewport picks use screen distance and source-surface or camera-target depth. Older unchanged ZoomExtents Top captures recover target depth from ray midpoints; newer depth captures record the target and camera direction independently. Parallel crossing projection pins the first supplied view normal. Perspective crossing paths and changing views remain unqualified.',
                    'Accepted points are staged until Enter. Cancel, Escape and another command discard them. Empty or Undo-all acceptance creates no geometry history and preserves redo. Stale source geometry, tolerance or editability rejects admission.',
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
