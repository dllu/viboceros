"""Qualify subdivision ellipsoid construction, sampled facets and owned history."""
from .qualification_sources import with_application_sources
import argparse
import copy
import math
import hashlib
import json
from pathlib import Path
import re
from .report_subd_box import ROOT, HERE, SOURCES as BASE_SOURCES, error
from .report_subd_cylinder_references import without_selection

FAMILIES = [('subd_ellipsoid', 50), ('subd_ellipsoid_details', 14)]
EXPLORATORY = [('subd_ellipsoid_discovery', 16), ('subd_ellipsoid_options', 18),
               ('subd_ellipsoid_shapes', 14), ('subd_ellipsoid_detail_diagnostics', 13)]
GETTER_ONLY = {'subd_ellipsoid_retry_second'}
SOURCES = list(dict.fromkeys(BASE_SOURCES + [
    'crates/viboceros-geometry/src/subd/ellipsoid.rs',
    'crates/viboceros-geometry/src/mesh.rs', 'crates/viboceros-geometry/src/ellipse.rs',
    'crates/viboceros-command/src/ellipse_input.rs',
    'crates/viboceros-command/src/curve_reference.rs',
    'crates/viboceros-command/src/subd_ellipsoid.rs',
    'crates/viboceros-command/src/subd_ellipsoid/input.rs',
    'crates/viboceros-command/src/subd_ellipsoid/arguments.rs',
    'crates/viboceros-command/tests/subd_ellipsoid_native.rs',
    'crates/viboceros-command/tests/support/subd_ellipsoid.rs',
    'crates/viboceros-command/tests/support/subd_facets.rs',
    'src/app/ellipse.rs', 'src/app/ellipse/guides.rs',
    'src/app/subd_ellipsoid.rs', 'src/app/subd_ellipsoid/tests.rs',
    'tools/rhino_oracle/subd_ellipsoid_probe.py',
    'tools/rhino_oracle/generate_subd_ellipsoid.py',
    'tools/rhino_oracle/report_subd_ellipsoid.py',
    'tools/rhino_oracle/test_subd_ellipsoid.py',
] + [f'tools/rhino_oracle/{directory}/{family}.json'
     for family, _ in FAMILIES + EXPLORATORY for directory in ['fixtures', 'observations']]))
SOURCES = with_application_sources(SOURCES)


def native_history_error(before, after):
    """Require exact restored data, allowing bounded evaluator-only roundoff."""
    a, b = copy.deepcopy(before), copy.deepcopy(after)
    if len(a) != len(b):
        raise ValueError('native ellipsoid history object count differs')
    maximum = 0.
    def points(p, q):
        nonlocal maximum
        if len(p) != len(q):
            raise ValueError('native ellipsoid history sample count differs')
        for x, y in zip(p, q):
            if len(x) != 3 or len(y) != 3 or any(type(v) not in (int, float) or not math.isfinite(v) for v in x + y):
                raise ValueError('invalid native ellipsoid history sample')
            maximum = max(maximum, math.dist(x, y))
    for p, q in zip(a, b):
        if 'subd' in p and 'subd' in q:
            points(p['subd'].pop('surface_points'), q['subd'].pop('surface_points'))
            pm, qm = p['subd']['surface_mesh'], q['subd']['surface_mesh']
            points(pm.pop('vertices'), qm.pop('vertices'))
    if a != b or maximum > 1e-6:
        raise ValueError('native ellipsoid history data or evaluated geometry differs')
    return maximum


def observations():
    expected, engines = {}, set()
    for family, count in FAMILIES:
        q = json.loads((HERE / 'fixtures' / (family + '.json')).read_text())
        native = json.loads((HERE / 'observations' / (family + '.json')).read_text())
        rows = native['results']
        if len(rows) != count or [r['id'] for r in rows] != [o['id'] for o in q['operations']]:
            raise ValueError('missing or reordered native ellipsoid recipes')
        engines.add(native['engine_version'])
        for row in rows:
            key, v = row['id'], row['value']
            success = not (v['input'].get('cancel') or v['input'].get('no_output'))
            outputs = [o for o in v['after_script'] if o['role'] == 'output']
            if key in expected or 'Unknown command:' in v['history'] or v['success'] is not success:
                raise ValueError('invalid native ellipsoid construction')
            if len(outputs) != int(success) or any('subd' not in o for o in outputs):
                raise ValueError('unexpected native ellipsoid outputs or focal markers')
            if without_selection(v['before']) != without_selection(v['undo']):
                raise ValueError('native ellipsoid sources or history differ')
            native_history_error(v['after_script'], v['redo'])
            if sum(e['name'] == 'SubDEllipsoid' for e in v['events']) != 1:
                raise ValueError('missing native ellipsoid lifecycle')
            expected[key] = v
    if len(engines) != 1:
        raise ValueError('mixed native ellipsoid engines')
    return expected, engines.pop()


def summarize(text):
    expected, engine = observations()
    stages, control_error, facet_error = {}, 0., 0.
    for stage in ['COMMAND', 'GETTER']:
        ids = [k for k, v in expected.items() if stage == 'GETTER' or not v['input'].get('cancel') and k not in GETTER_ONLY]
        rows = [json.loads(s) for s in re.findall(r'^SUBD_ELLIPSOID_' + stage + r' (.+)$', text, re.M)]
        if len(rows) != len(ids) or len({r['id'] for r in rows}) != len(ids) or {r['id'] for r in rows} != set(ids):
            raise ValueError('missing, duplicate or unrelated ellipsoid ' + stage)
        found = {r['id']: r for r in rows}
        rows = [found[k] for k in ids]
        for row in rows:
            value, check = expected[row['id']], row['check']
            targets = [o['subd'] for o in value['after_script'] if 'subd' in o]
            if row.get('grouped_history') is not True or check.get('qualified') is not True or check.get('attributes_selection_and_source') is not True or len(check['objects']) != len(targets):
                raise ValueError('unqualified ellipsoid source, attributes, selection or history')
            if stage == 'COMMAND' and row.get('declined') is not bool(value['input'].get('no_output')):
                raise ValueError('unqualified zero-axis ellipsoid admission')
            for actual, target in zip(check['objects'], targets):
                if target.get('valid') is not True or any(actual.get(k) is not True for k in ['qualified', 'surface_sampled', 'surface_facets_correspond', 'common_facet_triangulation']):
                    raise ValueError('unqualified ellipsoid geometry or facets')
                for key, values in [('vertices', target['points']), ('edges', target['edges']), ('faces', target['faces']), ('compared_surface_faces', target['surface_mesh']['faces'])]:
                    if type(actual.get(key)) is not int or actual[key] != len(values):
                        raise ValueError('ellipsoid topology or facet count differs')
                control_error = max(control_error, error(actual.get('maximum_control_or_limit_point_error')))
                facet_error = max(facet_error, error(actual.get('maximum_sampled_surface_error')))
        stages[stage.lower()] = rows
    diagnostics = []
    for family, count in EXPLORATORY:
        native = json.loads((HERE / 'observations' / (family + '.json')).read_text())
        if len(native['results']) != count:
            raise ValueError('missing exploratory ellipsoid recipes')
        diagnostics.extend(dict(id=r['id'], native_success=r['value']['success'], rejected_option='Unknown command:' in r['value']['history']) for r in native['results'])
    return dict(
        model_epsilon=1e-6, captured_recipes=64, successful_constructions=53,
        command_cases=54, getter_cases=64, zero_axis_cases=2, getter_only_recoveries=1,
        native_engine_version=engine, full_native_parity=False, performance_qualified=False,
        maximum_native_history_evaluation_error=max(native_history_error(v['after_script'], v['redo']) for v in expected.values()),
        maximum_control_or_limit_point_error=control_error, maximum_sampled_surface_error=facet_error,
        stages=stages, exploratory_recipes=diagnostics,
        guarantees=[
            'SubDEllipsoid reuses the rational MeshEllipsoid sampler and public OpenNURBS evaluation. Ordinary cages compensate local extents using density-two limit spans. Identity placement retains double source bounds; transformed placement follows the native float-bound path. Odd counts retain triangular caps; even counts support triangle and quad pole fans.',
            'Corner, centered Corner, Diameter, Diameter Vertical, locked axis lengths, spatial axes, tilted CPlanes, projected AroundCurve picks and signed third sizes match captured controls, tags, winding, vertex limit stations and density-two facet corners. Numeric diameters halve the second and third sizes; picked distances are unhalved.',
            'FromFoci completes a spheroid immediately. Its compensation uses density-three spans in the captured CPlane and scales about that reference origin, preserving native center shifts and axis changes for translated or spatial input. MarkFoci is independently remembered, but these native SubDEllipsoid captures produce no point objects.',
            'Numeric zero denotes the original CPlane origin at the center, first axis and third axis. At the second axis it is an invalid length and leaves the getter staged. The corrected recovery capture supplies a new length and direction; complete scripts reject its invalid token atomically.',
            'Shared ellipse picking retains source identity, layer, name and selection. Previews and cancellation preserve model history and pending Redo; history, tolerance and guide edits invalidate both parent and base getters. Successful construction has one object and one grouped Undo/Redo, with exact restored document objects checked independently of evaluation rounding.',
            'Every corresponding density-two quad has an oriented one-to-one corner match under a common triangulation. The reported facet bound uses corresponding barycentric points; native renderer diagonal choices and the continuous smooth surface are not certified by this check.',
        ],
        limits=[
            'The 61 exploratory recipes preserve raw histories and archived producers and are excluded from supported totals. The first detail batch lacked explicit cancellation after an invalid second-axis zero. Its corrected capture includes cancellation and a separate successful retry.',
            'At least three around and two vertical faces, at most 100000 control vertices/faces, and at most 1000000 evaluated facets. Density-three focal compensation can reach the facet budget earlier. Unrepresentable float extents are declined before admission.',
            'Bare Radius, initial Vertical, late topology options and Deformable are unsupported primitive options in the captured build. Vertical is supported inside Diameter. Enter cancels the third-axis prompt; a zero third measurement ends without geometry.',
            'Arbitrary option timing, every guide type and fit seam, native packing, material/texture channels and comparative performance remain unqualified. Sampled facet correspondence is not a continuous Hausdorff certificate.',
        ],
        sha256={p: hashlib.sha256((ROOT / p).read_bytes()).hexdigest() for p in SOURCES},
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('logs', nargs='+', type=Path)
    args = parser.parse_args()
    result = summarize('\n'.join(p.read_text() for p in args.logs))
    (ROOT / 'docs/subd-ellipsoid-qualification.json').write_text(json.dumps(result, indent=2, allow_nan=False) + '\n')


if __name__ == '__main__':
    main()
