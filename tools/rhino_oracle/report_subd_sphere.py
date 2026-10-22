"""Qualify sphere styles and reference getters against fixed public Rhino captures."""
from .qualification_sources import with_application_sources
import argparse
import hashlib
import json
from pathlib import Path
import re
from .report_subd_box import ROOT, HERE, SOURCES as BASE_SOURCES, error
from .report_subd_cylinder_references import without_selection

FAMILIES = [('subd_sphere', 29), ('subd_sphere_references', 25)]
DIAGNOSTICS = [('subd_sphere_discovery', 8), ('subd_sphere_styles', 6)]
SOURCES = list(dict.fromkeys(BASE_SOURCES + [
    'crates/viboceros-geometry/src/subd/sphere.rs',
    'crates/viboceros-geometry/src/subd/interpolation.rs',
    'crates/viboceros-geometry/src/mesh.rs',
    'crates/viboceros-command/src/subd_sphere.rs',
    'crates/viboceros-command/src/sphere_input.rs',
    'crates/viboceros-command/src/splop/reference.rs',
    'crates/viboceros-command/src/circle_input.rs',
    'crates/viboceros-command/src/circle_tangent_input.rs',
    'crates/viboceros-command/src/circle_around_input.rs',
    'crates/viboceros-command/src/point_fitting.rs',
    'crates/viboceros-command/tests/subd_sphere_native.rs',
    'crates/viboceros-command/tests/subd_sphere_references_native.rs',
    'crates/viboceros-command/tests/support/subd_sphere.rs',
    'crates/viboceros-command/tests/support/subd_sphere_references.rs',
    'crates/viboceros-command/tests/support/subd_cylinder_references.rs',
    'src/app/subd_sphere.rs', 'src/app/subd_sphere/tests.rs',
    'src/app/tangent.rs', 'src/app/circle_around.rs', 'src/app/object_selection.rs',
    'tools/rhino_oracle/subd_sphere_probe.py',
    'tools/rhino_oracle/subd_sphere_references_probe.py',
    'tools/rhino_oracle/subd_cylinder_references_probe.py',
    'tools/rhino_oracle/generate_subd_sphere.py',
    'tools/rhino_oracle/generate_subd_sphere_references.py',
    'tools/rhino_oracle/client.py', 'tools/rhino_oracle/conic_getter_input.py',
    'tools/rhino_oracle/report_subd_sphere.py', 'tools/rhino_oracle/test_subd_sphere.py',
] + [f'tools/rhino_oracle/{directory}/{family}.json'
     for family, _ in FAMILIES + DIAGNOSTICS for directory in ['fixtures', 'observations']]))
SOURCES = with_application_sources(SOURCES)


def observations():
    expected, engines = {}, set()
    for family, count in FAMILIES:
        request = json.loads((HERE / 'fixtures' / (family + '.json')).read_text())
        native = json.loads((HERE / 'observations' / (family + '.json')).read_text())
        if len(native['results']) != count or [r['id'] for r in native['results']] != [o['id'] for o in request['operations']]:
            raise ValueError('missing or reordered native sphere observations')
        engines.add(native['engine_version'])
        rows = expected[family] = {}
        for row in native['results']:
            value = row['value']
            if row['id'] in rows or 'Unknown command:' in value['history']:
                raise ValueError('duplicate or rejected native sphere input')
            if value['after_script'] != value['redo'] or without_selection(value['before']) != without_selection(value['undo']):
                raise ValueError('native sphere history or sources differ')
            if value['success'] is not (not value['input'].get('cancel', False)):
                raise ValueError('unexpected native sphere result')
            outputs = [o for o in value['after_script'] if 'subd' in o]
            if len(outputs) != int(not (value['input'].get('cancel') or value['input'].get('no_output'))):
                raise ValueError('native sphere output differs')
            for trace in value.get('input_trace', []):
                if trace['completed'] != trace['expected'] or trace['errors']:
                    raise ValueError('incomplete native sphere input')
                for click in trace.get('clicks', []):
                    if click.get('motion_acknowledged') is not True or click.get('click_acknowledged') is not True or not isinstance(click.get('projected_point'), list):
                        raise ValueError('missing native physical contact receipt')
            rows[row['id']] = value
    if len(engines) != 1:
        raise ValueError('mixed native sphere engines')
    return expected, engines.pop()


def summarize(text):
    expected, engine = observations()
    stages, maximum_control, maximum_surface = {}, 0., 0.
    for family, prefix, attributes in [
        ('subd_sphere', 'SUBD_SPHERE_', 'attributes_selection_and_source'),
        ('subd_sphere_references', 'SUBD_SPHERE_REFERENCE_', 'attributes_selection_grips_and_source'),
    ]:
        for stage in ['COMMAND', 'GETTER']:
            rows = [json.loads(s) for s in re.findall('^' + prefix + stage + r' (.+)$', text, re.M)]
            ids = [k for k, v in expected[family].items() if stage == 'GETTER' or not v['input'].get('cancel')]
            if len(rows) != len(ids) or len({r['id'] for r in rows}) != len(ids) or {r['id'] for r in rows} != set(ids):
                raise ValueError('missing, duplicate or unrelated sphere ' + family + stage)
            by_id = {r['id']: r for r in rows}
            rows = [by_id[k] for k in ids]
            for row in rows:
                check = row['check']
                targets = [o['subd'] for o in expected[family][row['id']]['after_script'] if 'subd' in o]
                if row.get('grouped_history') is not True or check.get('qualified') is not True or check.get(attributes) is not True or len(check['objects']) != len(targets):
                    raise ValueError('unqualified sphere source, selection, attributes or history')
                for actual, target in zip(check['objects'], targets):
                    if actual.get('qualified') is not True or actual.get('surface_sampled') is not True or target.get('valid') is not True:
                        raise ValueError('unqualified sphere geometry')
                    for name, values in [('vertices', target['points']), ('edges', target['edges']), ('faces', target['faces'])]:
                        if type(actual.get(name)) is not int or actual[name] != len(values):
                            raise ValueError('sphere topology differs')
                    maximum_control = max(maximum_control, error(actual.get('maximum_control_or_limit_point_error')))
                    maximum_surface = max(maximum_surface, error(actual.get('maximum_sampled_surface_error')))
            stages[('reference_' if family.endswith('references') else '') + stage.lower()] = rows
    diagnostics = []
    for family, count in DIAGNOSTICS:
        native = json.loads((HERE / 'observations' / (family + '.json')).read_text())
        if len(native['results']) != count:
            raise ValueError('missing native sphere diagnostics')
        diagnostics.extend(dict(id=r['id'], rejected_option='Unknown command:' in r['value']['history']) for r in native['results'])
    return dict(
        model_epsilon=1e-6, captured_recipes=54, successful_constructions=47,
        command_cases=49, getter_cases=54, successful_no_output_fits=2,
        native_engine_version=engine, maximum_control_or_limit_point_error=maximum_control,
        maximum_sampled_surface_error=maximum_surface, full_native_parity=False,
        performance_qualified=False, stages=stages, excluded_diagnostics=diagnostics,
        guarantees=[
            'Quad, Triangle and UV nets reuse sphere mesh samplers and public OpenNURBS subdivision evaluation. Quad refinement projects subdivided limit stations onto the sphere; UV nets interpolate latitude stations; triangular nets use a rotated icosahedron and uniform control radii.',
            'Captured center/radius, spatial radius point, Diameter, Circumference, Area, 2Point, 3Point, 4Point, section Radius, Orientation and tilted CPlane constructions match control topology, winding, tags, vertex limit stations and density-two sampled limit surfaces.',
            'Tangent uses the ordinary circle-contact rule, including a disjoint circle/line pair. Splop retains its distinct minimum-gap reference-sphere rule. Fitting accepts points and displayed selected grips, including rational controls, mixed inputs and partial preselection.',
            'AroundCurve accepts straight or circular guides and preselection. Physical native contacts retain acknowledged motion/clicks and projected world positions; replay uses admitted contacts.',
            'The getter stages construction outside the document. Completion adds one unselected SubD on the current layer in one Undo record. Source storage, names, layers, selection, grips and real Undo/Redo are compared.',
            'Collinear fits succeed without output or a modeling Undo record, preserving preselection and releasing postselection. Cancellation, stale inputs, invalid topology and failed admission preserve pending Redo.',
        ],
        limits=[
            'The 14 raw discovery/style recipes include rejected option guesses and fallback geometry. They are retained as diagnostics and excluded from supported comparisons.',
            'Native comparative speed, packing, textures/material channels, arbitrary fit seam/axis choices, surface-edge guides and all option clocks are unqualified.',
            'Quad subdivisions are limited to 0..6, Triangle to 0..5 and UV grids to at least 3 by 2, with a budget of 100000 control vertices and faces. High resolution interpolation cost has not been benchmarked.',
            'Hover previews show construction great circles. Subdivision spheres approximate an analytic sphere; facet sampling is not a continuous Hausdorff certificate.',
        ],
        sha256={p: hashlib.sha256((ROOT / p).read_bytes()).hexdigest() for p in SOURCES},
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('logs', nargs='+', type=Path)
    args = parser.parse_args()
    report = summarize('\n'.join(p.read_text() for p in args.logs))
    (ROOT / 'docs/subd-sphere-qualification.json').write_text(json.dumps(report, indent=2, allow_nan=False) + '\n')


if __name__ == '__main__':
    main()
