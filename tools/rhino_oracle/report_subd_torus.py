"""Qualify torus control nets, staged inputs and source-preserving references."""
import argparse
import hashlib
import json
from pathlib import Path
import re
from .report_subd_cylinder_references import ROOT, HERE, SOURCES as BASE_SOURCES, without_selection
from .report_subd_box import error

FAMILIES = [('subd_torus', 48), ('subd_torus_references', 29)]
EXPLORATORY = [('subd_torus_discovery', 13), ('subd_torus_sizes', 13),
               ('subd_torus_options', 12), ('subd_torus_reference_diagnostics', 29)]
CANCELLATIONS = 'tools/rhino_oracle/observations/subd_torus_reference_diagnostic_cancellations.json'
GETTER_ONLY = {'subd_torus_default_small_retry'}
SOURCES = list(dict.fromkeys(BASE_SOURCES + [
    'crates/viboceros-geometry/src/subd/torus.rs',
    'crates/viboceros-geometry/src/mesh.rs',
    'crates/viboceros-command/src/subd_torus.rs',
    'crates/viboceros-command/src/subd_torus/input.rs',
    'crates/viboceros-command/src/subd_torus/arguments.rs',
    'crates/viboceros-command/src/subd_torus/tube.rs',
    'crates/viboceros-command/tests/subd_torus_native.rs',
    'crates/viboceros-command/tests/subd_torus_references_native.rs',
    'crates/viboceros-command/tests/support/subd_torus.rs',
    'crates/viboceros-command/tests/support/subd_torus_references.rs',
    'crates/viboceros-command/tests/support/subd_facets.rs',
    'src/app/subd_torus.rs', 'src/app/subd_torus/tests.rs',
    'src/app/circle_around.rs',
    'tools/rhino_oracle/subd_torus_probe.py',
    'tools/rhino_oracle/subd_torus_references_probe.py',
    'tools/rhino_oracle/generate_subd_torus.py',
    'tools/rhino_oracle/generate_subd_torus_references.py',
    'tools/rhino_oracle/report_subd_torus.py',
    'tools/rhino_oracle/test_subd_torus.py', CANCELLATIONS,
] + [f'tools/rhino_oracle/{directory}/{family}.json'
     for family, _ in FAMILIES + EXPLORATORY for directory in ['fixtures', 'observations']]))


def validate_input_trace(value):
    traces = value.get('input_trace', [])
    contacts = sum(isinstance(a, dict) and 'pick' in a for a in value['input']['actions'])
    if len(traces) != int(bool(value['input'].get('live'))) or sum(len(t.get('clicks', [])) for t in traces) != contacts:
        raise ValueError('missing native torus input or contact trace')
    for trace in traces:
        if trace['expected'] != len(value['input']['actions']) or trace['completed'] != trace['expected'] or trace['errors'] != []:
            raise ValueError('incomplete native torus input')
        for click in trace.get('clicks', []):
            if click.get('motion_acknowledged') is not True or click.get('click_acknowledged') is not True or not isinstance(click.get('projected_point'), list):
                raise ValueError('missing native contact receipt')


def observations():
    expected, engines = {}, set()
    for family, count in FAMILIES:
        q = json.loads((HERE / 'fixtures' / (family + '.json')).read_text())
        native = json.loads((HERE / 'observations' / (family + '.json')).read_text())
        rows = native['results']
        if len(rows) != count or [r['id'] for r in rows] != [o['id'] for o in q['operations']]:
            raise ValueError('missing or reordered native torus observations')
        engines.add(native['engine_version'])
        found = expected[family] = {}
        for row in rows:
            key, value = row['id'], row['value']
            if key in found or 'Unknown command:' in value['history']:
                raise ValueError('unexpected or duplicate native torus recipe')
            if value['after_script'] != value['redo'] or without_selection(value['before']) != without_selection(value['undo']):
                raise ValueError('native torus history or sources differ')
            success = not (value['input'].get('cancel') or value['input'].get('no_output'))
            if value['success'] is not success or sum('subd' in o for o in value['after_script']) != int(success):
                raise ValueError('native torus result differs')
            validate_input_trace(value)
            found[key] = value
    if len(engines) != 1:
        raise ValueError('mixed native torus engines')
    return expected, engines.pop()


def summarize(text):
    expected, engine = observations()
    stages, maximum_control, maximum_surface = {}, 0., 0.
    for family, prefix, attributes in [
        ('subd_torus', 'SUBD_TORUS_', 'attributes_selection_and_source'),
        ('subd_torus_references', 'SUBD_TORUS_REFERENCE_', 'attributes_selection_grips_and_source'),
    ]:
        for stage in ['COMMAND', 'GETTER']:
            ids = [k for k, v in expected[family].items() if stage == 'GETTER' or not v['input'].get('cancel') and k not in GETTER_ONLY]
            rows = [json.loads(s) for s in re.findall('^' + prefix + stage + r' (.+)$', text, re.M)]
            if len(rows) != len(ids) or len({r['id'] for r in rows}) != len(ids) or {r['id'] for r in rows} != set(ids):
                raise ValueError('missing, duplicate or unrelated torus ' + family + stage)
            by_id = {r['id']: r for r in rows}
            rows = [by_id[k] for k in ids]
            for row in rows:
                value, check = expected[family][row['id']], row['check']
                targets = [o['subd'] for o in value['after_script'] if 'subd' in o]
                if row.get('grouped_history') is not True or check.get('qualified') is not True or check.get(attributes) is not True or len(check['objects']) != len(targets):
                    raise ValueError('unqualified torus sources, selection, attributes or history')
                if stage == 'COMMAND' and value['input'].get('no_output') and row.get('declined') is not True:
                    raise ValueError('unqualified degenerate torus admission')
                for actual, target in zip(check['objects'], targets):
                    if actual.get('qualified') is not True or actual.get('surface_sampled') is not True or target.get('valid') is not True or actual.get('surface_facets_correspond') is not True or type(actual.get('compared_surface_faces')) is not int or actual['compared_surface_faces'] != len(target['surface_mesh']['faces']):
                        raise ValueError('unqualified torus geometry')
                    for name, values in [('vertices', target['points']), ('edges', target['edges']), ('faces', target['faces'])]:
                        if type(actual.get(name)) is not int or actual[name] != len(values):
                            raise ValueError('torus topology differs')
                    maximum_control = max(maximum_control, error(actual.get('maximum_control_or_limit_point_error')))
                    maximum_surface = max(maximum_surface, error(actual.get('maximum_sampled_surface_error')))
            stages[('reference_' if family.endswith('references') else '') + stage.lower()] = rows
    diagnostics = []
    for family, count in EXPLORATORY:
        native = json.loads((HERE / 'observations' / (family + '.json')).read_text())
        if len(native['results']) != count:
            raise ValueError('missing exploratory torus recipes')
        diagnostics.extend(dict(id=r['id'], rejected_option='Unknown command:' in r['value']['history'], native_success=r['value']['success']) for r in native['results'])
    return dict(
        model_epsilon=1e-6, captured_recipes=77, successful_constructions=58,
        command_cases=59, getter_cases=77, zero_base_fits=2, getter_only_recoveries=1,
        native_engine_version=engine, maximum_control_or_limit_point_error=maximum_control,
        maximum_sampled_surface_error=maximum_surface, full_native_parity=False,
        performance_qualified=False, stages=stages, exploratory_recipes=diagnostics,
        diagnostic_cancellations=json.loads((ROOT / CANCELLATIONS).read_text()),
        guarantees=[
            'SubDTorus reuses the polygon torus sampler and public OpenNURBS circular limit placement. The separable regular quad net interpolates the requested torus grid at vertex limit stations.',
            'Around and tube counts, odd grids, topology, winding, smooth tags, control points and vertex limit stations match captured outputs. Every density-two tessellation quad has a one-to-one oriented corner correspondence, with a bound including quad nonplanarity; this is not a continuous SubD surface certificate.',
            'Base Radius, Diameter, Circumference, Area, 2Point, spatial 3Point, Orientation, Vertical and tilted CPlanes match captured construction. The second Radius or Diameter halves numeric diameters; point input projects into the accepted base plane and measures absolute radial distance from the base circle regardless of size mode.',
            'FixInnerDimension=Yes adds tube radius to the base radius, preserving the inner dimension and accepting tubes larger than the base. Ordinary tubes require a positive radius strictly smaller than the base. Near-horn admission replays actual numeric macro tokens, including native string rounding, without changing epsilon.',
            'Base and tube size modes and defaults are independent. Numeric zero at the second prompt denotes the original CPlane origin. An invalid remembered tube remains staged for retry; this recovery is getter-only because complete scripts reject invalid input atomically.',
            'Shared tangent, curve-guided and fitted-point getters preserve source attributes, object and rational grip selection, and grouped Undo/Redo. Physical tangent receipts retain acknowledged motion, click and projected world contacts. Previews and cancellation preserve model history and pending Redo; history, tolerance or source changes invalidate drafts.',
            'Collinear and coincident FitPoints stage a zero-radius base. Ordinary positive tube input remains invalid; FixInnerDimension=Yes reaches geometry admission and ends without adding geometry. Both failure paths preserve sources and history.',
        ],
        limits=[
            'The 67 exploratory recipes retain raw histories and archived producer bytes and are excluded from supported comparisons. The first reference batch needed two externally sent Escape actions in its owned headless process; the supplemental cancellation artifact records these. Corrected reference macros cancel explicitly and need no external input.',
            'At least three faces in each direction and at most 100000 control vertices/faces. Arbitrary fit seams, option timing and surface-edge tangencies require further qualification. General horn and spindle command construction is unsupported.',
            'Native packing, material and texture channels and comparative performance remain unqualified. Preview outlines show construction circles; matching density-two facets is not a continuous Hausdorff certificate.',
        ],
        sha256={p: hashlib.sha256((ROOT / p).read_bytes()).hexdigest() for p in SOURCES},
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('logs', nargs='+', type=Path)
    args = parser.parse_args()
    report = summarize('\n'.join(p.read_text() for p in args.logs))
    (ROOT / 'docs/subd-torus-qualification.json').write_text(json.dumps(report, indent=2, allow_nan=False) + '\n')


if __name__ == '__main__':
    main()
