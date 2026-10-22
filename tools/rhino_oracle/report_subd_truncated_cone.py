"""Qualify truncated-cone control nets, staged inputs and source-preserving references."""
from .qualification_sources import with_application_sources
import argparse
import hashlib
import json
from pathlib import Path
import re
from .report_subd_cylinder_references import ROOT, HERE, SOURCES as BASE_SOURCES, without_selection
from .report_subd_box import error

FAMILIES = [('subd_truncated_cone', 33), ('subd_truncated_cone_references', 25)]
EXPLORATORY = [('subd_truncated_cone_discovery', 18), ('subd_truncated_cone_unordered', 33)]
SOURCES = list(dict.fromkeys(BASE_SOURCES + [
    'crates/viboceros-geometry/src/subd/truncated_cone.rs',
    'crates/viboceros-geometry/src/subd/cone.rs',
    'crates/viboceros-geometry/src/subd/interpolation.rs',
    'crates/viboceros-command/src/subd_axial/end_circle.rs',
    'crates/viboceros-geometry/src/mesh.rs',
    'crates/viboceros-command/src/subd_truncated_cone.rs',
    'crates/viboceros-command/tests/subd_truncated_cone_native.rs',
    'crates/viboceros-command/tests/subd_truncated_cone_references_native.rs',
    'crates/viboceros-command/tests/support/subd_truncated_cone.rs',
    'crates/viboceros-command/tests/support/subd_truncated_cone_references.rs',
    'src/app/subd_axial/truncated_tests.rs',
    'tools/rhino_oracle/subd_truncated_cone_probe.py',
    'tools/rhino_oracle/subd_truncated_cone_references_probe.py',
    'tools/rhino_oracle/generate_subd_truncated_cone.py',
    'tools/rhino_oracle/generate_subd_truncated_cone_references.py',
    'tools/rhino_oracle/report_subd_truncated_cone.py',
    'tools/rhino_oracle/test_subd_truncated_cone.py',
] + [f'tools/rhino_oracle/{directory}/{family}.json'
     for family, _ in FAMILIES + EXPLORATORY for directory in ['fixtures', 'observations']]))
SOURCES = with_application_sources(SOURCES)


def observations():
    expected, engines = {}, set()
    for family, count in FAMILIES:
        q = json.loads((HERE / 'fixtures' / (family + '.json')).read_text())
        native = json.loads((HERE / 'observations' / (family + '.json')).read_text())
        rows = native['results']
        if len(rows) != count or [r['id'] for r in rows] != [o['id'] for o in q['operations']]:
            raise ValueError('missing or reordered native truncated_cone observations')
        engines.add(native['engine_version'])
        found = expected[family] = {}
        for row in rows:
            key, value = row['id'], row['value']
            if key in found or 'Unknown command:' in value['history']:
                raise ValueError('unexpected or duplicate native truncated_cone recipe')
            if value['after_script'] != value['redo'] or without_selection(value['before']) != without_selection(value['undo']):
                raise ValueError('native truncated_cone history or sources differ')
            success = not (value['input'].get('cancel') or value['input'].get('no_output'))
            if value['success'] is not success or sum('subd' in o for o in value['after_script']) != int(success):
                raise ValueError('native truncated_cone result differs')
            for trace in value.get('input_trace', []):
                if trace['completed'] != trace['expected'] or trace['errors'] != []:
                    raise ValueError('incomplete native truncated_cone input')
                for click in trace.get('clicks', []):
                    if click.get('motion_acknowledged') is not True or click.get('click_acknowledged') is not True or not isinstance(click.get('projected_point'), list):
                        raise ValueError('missing native contact receipt')
            found[key] = value
    if len(engines) != 1:
        raise ValueError('mixed native truncated_cone engines')
    return expected, engines.pop()


def summarize(text):
    expected, engine = observations()
    stages, maximum_control, maximum_surface = {}, 0., 0.
    for family, prefix, attributes in [
        ('subd_truncated_cone', 'SUBD_TRUNCATED_CONE_', 'attributes_selection_and_source'),
        ('subd_truncated_cone_references', 'SUBD_TRUNCATED_CONE_REFERENCE_', 'attributes_selection_grips_and_source'),
    ]:
        for stage in ['COMMAND', 'GETTER']:
            ids = [k for k, v in expected[family].items() if stage == 'GETTER' or not v['input'].get('cancel')]
            rows = [json.loads(s) for s in re.findall('^' + prefix + stage + r' (.+)$', text, re.M)]
            if len(rows) != len(ids) or len({r['id'] for r in rows}) != len(ids) or {r['id'] for r in rows} != set(ids):
                raise ValueError('missing, duplicate or unrelated truncated_cone ' + family + stage)
            by_id = {r['id']: r for r in rows}
            rows = [by_id[k] for k in ids]
            for row in rows:
                value, check = expected[family][row['id']], row['check']
                targets = [o['subd'] for o in value['after_script'] if 'subd' in o]
                if row.get('grouped_history') is not True or check.get('qualified') is not True or check.get(attributes) is not True or len(check['objects']) != len(targets):
                    raise ValueError('unqualified truncated_cone sources, selection, attributes or history')
                if stage == 'COMMAND' and value['input'].get('no_output') and row.get('declined') is not True:
                    raise ValueError('unqualified degenerate truncated_cone admission')
                for actual, target in zip(check['objects'], targets):
                    if actual.get('qualified') is not True or actual.get('surface_sampled') is not True or target.get('valid') is not True:
                        raise ValueError('unqualified truncated_cone geometry')
                    for name, values in [('vertices', target['points']), ('edges', target['edges']), ('faces', target['faces'])]:
                        if type(actual.get(name)) is not int or actual[name] != len(values):
                            raise ValueError('truncated_cone topology differs')
                    maximum_control = max(maximum_control, error(actual.get('maximum_control_or_limit_point_error')))
                    maximum_surface = max(maximum_surface, error(actual.get('maximum_sampled_surface_error')))
            stages[('reference_' if family.endswith('references') else '') + stage.lower()] = rows
    diagnostics = []
    for family, count in EXPLORATORY:
        native = json.loads((HERE / 'observations' / (family + '.json')).read_text())
        if len(native['results']) != count:
            raise ValueError('missing exploratory truncated_cone recipes')
        diagnostics.extend(dict(id=r['id'], rejected_option='Unknown command:' in r['value']['history'], native_success=r['value']['success']) for r in native['results'])
    return dict(
        model_epsilon=1e-6, captured_recipes=58, successful_constructions=46,
        command_cases=46, getter_cases=58, zero_base_fits=2,
        native_engine_version=engine, maximum_control_or_limit_point_error=maximum_control,
        maximum_sampled_surface_error=maximum_surface, full_native_parity=False,
        performance_qualified=False, stages=stages, exploratory_recipes=diagnostics,
        guarantees=[
            'SubDTruncatedCone reuses the polygon mesh sampler and public OpenNURBS SubD construction. Capped frusta use smooth control-cage radii; open frusta interpolate ring limit stations. Zero ends reuse the cone sampler and sparse limit-position interpolation for open extraordinary apexes.',
            'Triangle fans, even quad caps, odd-count fallback, single triangular caps and four-sided Tri/Quad caps match captured topology, winding, edge and vertex tags.',
            'Base Radius, Diameter, Circumference, Area, 2Point, spatial 3Point, tilted CPlanes, signed height, constrained endpoint picks and free axes match control points, vertex limit stations and density-two limit surface samples.',
            'DirectionConstraint is available at the initial base prompt. The accepted height freezes the top measurement plane. Top Radius and Diameter accept numbers or projected points, including zero; their default retains the entered size rather than converting when switching mode.',
            'Shared axial command and viewport controllers preserve independent per-command preferences, source attributes, object and grip selection, and grouped Undo/Redo. Preview and cancellation leave model history and pending Redo intact; external history or tolerance edits invalidate drafts.',
            'Tangent curve and point contacts, a disjoint circle/line pair, fitted points, rational grips and partial or postselection retain sources. Physical tangent picks retain acknowledged mouse motions, clicks and projected world contacts.',
            'Collinear and coincident fits yield a zero-radius base. A positive end radius completes an uncapped reverse cone with control-cage radii even with Solid=Yes. Ordinary typed zero base radii remain invalid; zero top radius is supported.',
        ],
        limits=[
            'The 18 discovery recipes retain rejected late DirectionConstraint and unsupported options. A further 33 preliminary recipes retain size-mode choices affected by unordered helper iteration. Both diagnostic families preserve raw evidence and archived producers and are excluded from supported comparisons.',
            'At least three around and two vertical faces, with at most 100000 control vertices and faces. BothSides, angle heights, AroundCurve, Circle Orientation and top Area/Circumference are unsupported. General option clocks, arbitrary fit seam choices and surface-edge tangencies need further qualification.',
            'Native packing, material and texture channels and comparative performance remain unqualified. Preview outlines show construction geometry. Sampled facets are not a continuous Hausdorff certificate.',
        ],
        sha256={p: hashlib.sha256((ROOT / p).read_bytes()).hexdigest() for p in SOURCES},
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('logs', nargs='+', type=Path)
    args = parser.parse_args()
    report = summarize('\n'.join(p.read_text() for p in args.logs))
    (ROOT / 'docs/subd-truncated-cone-qualification.json').write_text(json.dumps(report, indent=2, allow_nan=False) + '\n')


if __name__ == '__main__':
    main()
