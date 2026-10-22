"""Qualify cone control nets, staged inputs and source-preserving references."""
from .qualification_sources import with_shared_sources
import argparse
import hashlib
import json
from pathlib import Path
import re
from .report_subd_cylinder_references import ROOT, HERE, SOURCES as BASE_SOURCES, without_selection
from .report_subd_box import error

FAMILIES = [('subd_cone', 32), ('subd_cone_references', 24)]
EXPLORATORY = [('subd_cone_discovery', 14), ('subd_cone_options', 15)]
REJECTED_REFERENCE = 'cone_tangent_base_angle'
SOURCES = list(dict.fromkeys(BASE_SOURCES + [
    'crates/viboceros-geometry/src/subd/cone.rs',
    'crates/viboceros-geometry/src/mesh.rs',
    'crates/viboceros-command/src/subd_cone.rs',
    'crates/viboceros-command/tests/subd_cone_native.rs',
    'crates/viboceros-command/tests/subd_cone_references_native.rs',
    'crates/viboceros-command/tests/support/subd_cone.rs',
    'crates/viboceros-command/tests/support/subd_cone_references.rs',
    'src/app/subd_axial/cone_tests.rs',
    'tools/rhino_oracle/subd_cone_probe.py',
    'tools/rhino_oracle/subd_cone_references_probe.py',
    'tools/rhino_oracle/generate_subd_cone.py',
    'tools/rhino_oracle/generate_subd_cone_references.py',
    'tools/rhino_oracle/report_subd_cone.py',
    'tools/rhino_oracle/test_subd_cone.py',
] + [f'tools/rhino_oracle/{directory}/{family}.json'
     for family, _ in FAMILIES + EXPLORATORY for directory in ['fixtures', 'observations']]))
SOURCES = with_shared_sources(SOURCES)


def observations():
    expected, engines = {}, set()
    for family, count in FAMILIES:
        q = json.loads((HERE / 'fixtures' / (family + '.json')).read_text())
        native = json.loads((HERE / 'observations' / (family + '.json')).read_text())
        rows = native['results']
        if len(rows) != count or [r['id'] for r in rows] != [o['id'] for o in q['operations']]:
            raise ValueError('missing or reordered native cone observations')
        engines.add(native['engine_version'])
        found = expected[family] = {}
        for row in rows:
            key, value = row['id'], row['value']
            rejected = key == REJECTED_REFERENCE
            if key in found or ('Unknown command:' in value['history']) != rejected:
                raise ValueError('unexpected or duplicate native cone recipe')
            if value['after_script'] != value['redo'] or without_selection(value['before']) != without_selection(value['undo']):
                raise ValueError('native cone history or sources differ')
            success = not (value['input'].get('cancel') or value['input'].get('no_output') or rejected)
            if value['success'] is not success or sum('subd' in o for o in value['after_script']) != int(success):
                raise ValueError('native cone result differs')
            for trace in value.get('input_trace', []):
                if trace['completed'] != trace['expected'] or trace['errors'] != (['owned SubDCone input timeout'] if rejected else []):
                    raise ValueError('incomplete native cone input')
                for click in trace.get('clicks', []):
                    if click.get('motion_acknowledged') is not True or click.get('click_acknowledged') is not True or not isinstance(click.get('projected_point'), list):
                        raise ValueError('missing native contact receipt')
            found[key] = value
    if len(engines) != 1:
        raise ValueError('mixed native cone engines')
    return expected, engines.pop()


def summarize(text):
    expected, engine = observations()
    stages, maximum_control, maximum_surface = {}, 0., 0.
    for family, prefix, attributes in [
        ('subd_cone', 'SUBD_CONE_', 'attributes_selection_and_source'),
        ('subd_cone_references', 'SUBD_CONE_REFERENCE_', 'attributes_selection_grips_and_source'),
    ]:
        for stage in ['COMMAND', 'GETTER']:
            ids = [k for k, v in expected[family].items() if k != REJECTED_REFERENCE and (stage == 'GETTER' or not v['input'].get('cancel'))]
            rows = [json.loads(s) for s in re.findall('^' + prefix + stage + r' (.+)$', text, re.M)]
            if len(rows) != len(ids) or len({r['id'] for r in rows}) != len(ids) or {r['id'] for r in rows} != set(ids):
                raise ValueError('missing, duplicate or unrelated cone ' + family + stage)
            by_id = {r['id']: r for r in rows}
            rows = [by_id[k] for k in ids]
            for row in rows:
                value, check = expected[family][row['id']], row['check']
                targets = [o['subd'] for o in value['after_script'] if 'subd' in o]
                if row.get('grouped_history') is not True or check.get('qualified') is not True or check.get(attributes) is not True or len(check['objects']) != len(targets):
                    raise ValueError('unqualified cone sources, selection, attributes or history')
                if stage == 'COMMAND' and value['input'].get('no_output') and row.get('declined') is not True:
                    raise ValueError('unqualified degenerate cone admission')
                for actual, target in zip(check['objects'], targets):
                    if actual.get('qualified') is not True or actual.get('surface_sampled') is not True or target.get('valid') is not True:
                        raise ValueError('unqualified cone geometry')
                    for name, values in [('vertices', target['points']), ('edges', target['edges']), ('faces', target['faces'])]:
                        if type(actual.get(name)) is not int or actual[name] != len(values):
                            raise ValueError('cone topology differs')
                    maximum_control = max(maximum_control, error(actual.get('maximum_control_or_limit_point_error')))
                    maximum_surface = max(maximum_surface, error(actual.get('maximum_sampled_surface_error')))
            stages[('reference_' if family.endswith('references') else '') + stage.lower()] = rows
    diagnostics = []
    for family, count in EXPLORATORY:
        native = json.loads((HERE / 'observations' / (family + '.json')).read_text())
        if len(native['results']) != count:
            raise ValueError('missing exploratory cone recipes')
        diagnostics.extend(dict(id=r['id'], rejected_option='Unknown command:' in r['value']['history'], native_success=r['value']['success']) for r in native['results'])
    return dict(
        model_epsilon=1e-6, captured_recipes=56, successful_constructions=44,
        command_cases=46, getter_cases=55, declined_degenerate_fits=2,
        native_engine_version=engine, maximum_control_or_limit_point_error=maximum_control,
        maximum_sampled_surface_error=maximum_surface, full_native_parity=False,
        performance_qualified=False, stages=stages, exploratory_recipes=diagnostics,
        rejected_reference_recipe=REJECTED_REFERENCE,
        guarantees=[
            'SubDCone reuses the polygonal MeshCone sampler and public OpenNURBS SubD construction. The apex and closed base are smooth; an open base has boundary crease tags. Dimensions locate the control cage rather than its rounded limit surface.',
            'Triangle fans, even quad caps, odd-count fallback and the native single three-sided cap are compared explicitly. A four-sided Tri cap retains its center/fan, while Quad uses one face.',
            'Center/radius, spatial radius points, Diameter, Circumference, Area, 2Point, spatial 3Point, tilted CPlanes, signed heights, constrained endpoint picks and free axes match topology, winding, tags, controls, vertex limit stations and density-two limit surface samples.',
            'Base and apex angle inputs convert signed degrees to height. They accept separate or assigned values; bases with tangent curve contacts reject them, while point-only tangent bases retain them.',
            'The shared axial controller has separate per-command preference instances. Cone starts with height 10, while Cylinder retains height 1. Point tools, previews, reference handoff, stale-source guards and one constructive Undo/Redo stay shared.',
            'Tangent point/curve contacts, a disjoint circle/line pair, point fitting, rational grips, partial preselection and postselection retain source storage, attributes, selection and grip display. Native physical picks replay acknowledged projected world contacts.',
            'Collinear/coincident fits reach the height prompt, then fail admission without creating an object. Cancellation and rejected drafts add no model history and do not consume pending Redo. External document edits retain their own effects on history.',
        ],
        limits=[
            'The captured Rhino build rejects BothSides, AroundCurve, Circle Orientation and VerticalFaces=1. Vertical resolves to VerticalFaces rather than a vertical circle. The 29 exploratory recipes retain these rejections and fallback effects, plus angle diagnostics; they are excluded from supported comparisons.',
            'The captured OrBaseAngle after Tangent is rejected and cancelled. The owned driver cancelled after its timeout. Its complete raw recipe remains in the primary reference capture and is excluded from supported geometry/getter comparisons.',
            'Counts require at least three around and two vertical faces, with at most 100000 control vertices and faces. General option clocks, arbitrary fit seam/normal choices, surface-edge tangencies and every mouse trajectory need further qualification.',
            'Native packing, texture/material channels and comparative performance are unqualified. Preview outlines show the construction cage; sampled facets are not a continuous Hausdorff certificate.',
        ],
        sha256={p: hashlib.sha256((ROOT / p).read_bytes()).hexdigest() for p in SOURCES},
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('logs', nargs='+', type=Path)
    args = parser.parse_args()
    report = summarize('\n'.join(p.read_text() for p in args.logs))
    (ROOT / 'docs/subd-cone-qualification.json').write_text(json.dumps(report, indent=2, allow_nan=False) + '\n')


if __name__ == '__main__':
    main()
