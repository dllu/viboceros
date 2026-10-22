"""Qualify subdivision-cylinder reference getters against owned Rhino captures."""
import argparse
import hashlib
import json
from pathlib import Path
import re
from .qualification_sources import with_shared_sources
from .report_subd_cylinder import ROOT, HERE, SOURCES as BASE_SOURCES
from .report_subd_box import error

FAMILIES = [('subd_cylinder_references', 20), ('subd_cylinder_reference_degenerate', 2)]
SOURCES = with_shared_sources(BASE_SOURCES + [
    'crates/viboceros-command/src/circle_fit_points.rs',
    'crates/viboceros-command/src/circle_tangent_input.rs',
    'crates/viboceros-command/src/circle_around_input.rs',
    'crates/viboceros-command/src/point_selection.rs',
    'crates/viboceros-command/src/object_selection.rs',
    'crates/viboceros-command/tests/subd_cylinder_references_native.rs',
    'crates/viboceros-command/tests/support/subd_cylinder_references.rs',
    'crates/viboceros-geometry/src/circle_tangent.rs',
    'crates/viboceros-geometry/src/circle_fit.rs',
    'crates/viboceros-geometry/src/circle_fit/seed.rs',
    'crates/viboceros-geometry/src/circle_fit/distant.rs',
    'crates/viboceros-geometry/src/finite_sum.rs',
    'crates/viboceros-document/src/selection.rs',
    'crates/viboceros-document/src/control_points.rs',
    'src/app/tangent.rs', 'src/app/object_selection.rs',
    'tools/rhino_oracle/client.py', 'tools/rhino_oracle/catenary_point_picker.py',
    'tools/rhino_oracle/conic_getter_input.py',
    'tools/rhino_oracle/managed_mouse_callback.py', 'tools/rhino_oracle/snap_environment.py',
    'tools/rhino_oracle/subd_cylinder_references_probe.py',
    'tools/rhino_oracle/generate_subd_cylinder_references.py',
    'tools/rhino_oracle/generate_subd_cylinder_reference_degenerate.py',
    'tools/rhino_oracle/report_subd_cylinder_references.py',
    'tools/rhino_oracle/test_subd_cylinder_references.py',
] + [f'tools/rhino_oracle/{directory}/{family}.json'
     for family, _ in FAMILIES for directory in ['fixtures', 'observations']])


def without_selection(rows):
    rows = json.loads(json.dumps(rows))
    for row in rows:
        row.pop('selected', None)
        for grip in row.get('grips', []):
            grip.pop('selected', None)
    return rows


def observations():
    expected = {}
    engines = set()
    for family, count in FAMILIES:
        request = json.loads((HERE / 'fixtures' / (family + '.json')).read_text())
        native = json.loads((HERE / 'observations' / (family + '.json')).read_text())
        rows = native['results']
        if len(rows) != count or [r['id'] for r in rows] != [o['id'] for o in request['operations']]:
            raise ValueError('missing or reordered native reference observations')
        engines.add(native['engine_version'])
        for row in rows:
            key, value = row['id'], row['value']
            if key in expected or 'Unknown command:' in value['history']:
                raise ValueError('duplicate or invalid native reference recipe')
            if value['after_script'] != value['redo'] or without_selection(value['before']) != without_selection(value['undo']):
                raise ValueError('native reference history or sources differ')
            should_succeed = not (value['input'].get('cancel') or value['input'].get('no_output'))
            if value['success'] is not should_succeed:
                raise ValueError('unexpected native reference command result')
            for trace in value['input_trace']:
                if trace['completed'] != trace['expected'] or trace['errors']:
                    raise ValueError('incomplete native contact input')
                if any(c.get('motion_acknowledged') is not True or c.get('click_acknowledged') is not True or not isinstance(c.get('projected_point'), list) for c in trace['clicks']):
                    raise ValueError('missing native physical contact receipt')
            expected[key] = value
    if len(engines) != 1:
        raise ValueError('mixed native engines')
    return expected, engines.pop()


def summarize(text):
    expected, engine = observations()
    stages = {}
    maximum_control = maximum_surface = 0.
    for stage in ['COMMAND', 'GETTER']:
        rows = [json.loads(s) for s in re.findall(r'^SUBD_CYLINDER_REFERENCE_' + stage + r' (.+)$', text, re.M)]
        ids = [k for k, v in expected.items() if stage == 'GETTER' or v['success'] or k.endswith('_finish')]
        # Separate Rust tests may report successful and declined commands in
        # either order. Fixed IDs, one occurrence each, determine correspondence.
        if len(rows) != len(ids) or len({r['id'] for r in rows}) != len(ids) or {r['id'] for r in rows} != set(ids):
            raise ValueError('missing, duplicate or unrelated reference ' + stage)
        by_id = {r['id']: r for r in rows}
        rows = [by_id[k] for k in ids]
        for row in rows:
            check = row['check']
            targets = [o['subd'] for o in expected[row['id']]['after_script'] if 'subd' in o]
            if row.get('grouped_history') is not True or check.get('qualified') is not True or check.get('attributes_selection_grips_and_source') is not True or len(check['objects']) != len(targets):
                raise ValueError('unqualified reference source, selection, attributes or history')
            if stage == 'COMMAND' and row['id'].endswith('_finish') and row.get('declined') is not True:
                raise ValueError('unqualified degenerate reference admission')
            for actual, target in zip(check['objects'], targets):
                if actual.get('qualified') is not True or actual.get('surface_sampled') is not True or target.get('valid') is not True:
                    raise ValueError('unqualified reference geometry')
                for name, values in [('vertices', target['points']), ('edges', target['edges']), ('faces', target['faces'])]:
                    if type(actual.get(name)) is not int or actual[name] != len(values):
                        raise ValueError('reference topology differs')
                maximum_control = max(maximum_control, error(actual.get('maximum_control_or_limit_point_error')))
                maximum_surface = max(maximum_surface, error(actual.get('maximum_sampled_surface_error')))
        stages[stage.lower()] = rows
    return dict(
        model_epsilon=1e-6, captured_recipes=22, successful_constructions=14,
        command_cases=16, getter_cases=22, native_engine_version=engine,
        maximum_control_or_limit_point_error=maximum_control,
        maximum_sampled_surface_error=maximum_surface,
        full_native_parity=False, performance_qualified=False, stages=stages,
        guarantees=[
            'SubDCylinder reuses the existing read-only tangent-contact getter and spatial least-squares circle fitter. Reference circles stay outside the document until the cylinder is complete.',
            'Three point contacts, spatial contacts, two curve contacts, a fixed radius, late Radius, FromFirstPoint and three line contacts match captured topology, tags, oriented height, control points, vertex limit stations and sampled limit surfaces.',
            'FitPoints accepts point objects and displayed selected control points, including rational curve controls and mixed point/grip sources. Partial preselection, postselection and native source selection on Undo/Redo are compared explicitly.',
            'Physical native curve picks retain acknowledged mouse motion, click receipts and projected world contacts. Command and getter comparisons use those admitted contacts rather than the requested screen aim.',
            'Cancellation and collinear/coincident fit failure preserve source geometry, attributes, grip display and Redo. A zero-radius fit still requests height, then fails admission without adding an object.',
            'Completion adds one unselected SubD on the current layer in one Undo record. The shared tangent picker also retains its existing Circle, Arc and Splop recipients.',
        ],
        limits=[
            'Tangent curve captures use straight planar guides. Curved tangencies, surface edges, other contact combinations and option sequences need further cylinder-specific qualification.',
            'General point-fit circle axes and oriented normals are solver-dependent. Captures qualify these selected point sets; arbitrary fitted SubD seam/normal parity is not established.',
            'Point fitting uses the existing point-count limit and convergence checks. Construction is limited to 100000 control vertices and faces.',
            'Limit-surface facet sampling is not a continuous Hausdorff certificate. Comparative speed, native packing and texture/material parity remain unqualified.',
        ],
        sha256={p: hashlib.sha256((ROOT / p).read_bytes()).hexdigest() for p in SOURCES},
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('logs', nargs='+', type=Path)
    args = parser.parse_args()
    report = summarize('\n'.join(p.read_text() for p in args.logs))
    (ROOT / 'docs/subd-cylinder-references-qualification.json').write_text(json.dumps(report, indent=2, allow_nan=False) + '\n')


if __name__ == '__main__':
    main()
