"""Qualify fixed subdivision workflows; reject incomplete or invalid comparisons."""
from .qualification_sources import with_shared_sources
import argparse
import hashlib
import json
import re
from pathlib import Path
from .report_subd_box import ROOT, HERE, SOURCES as BASE_SOURCES, error
from .report_subd_ellipsoid import native_history_error

FAMILIES = [('subdivide', 41), ('subdivide_details', 17),
            ('subdivide_orientations', 74), ('subdivide_extra', 4), ('subdivide_ngons', 5)]
EXPLORATORY = [('subdivide_discovery', 16)]
SOURCES = list(dict.fromkeys(BASE_SOURCES + [
    'crates/viboceros-opennurbs/native/subd_subdivision.h',
    'crates/viboceros-opennurbs/build.rs',
    'crates/viboceros-command/src/object_selection.rs',
    'crates/viboceros-geometry/src/error.rs',
    'crates/viboceros-geometry/src/mesh.rs',
    'src/app/untrim_holes.rs',
    'crates/viboceros-command/src/subdivide.rs',
    'crates/viboceros-command/src/subdivide/arguments.rs',
    'crates/viboceros-command/src/component_selection.rs',
    'crates/viboceros-command/src/geometry_selection.rs',
    'crates/viboceros-command/tests/subdivide_native.rs',
    'crates/viboceros-command/tests/subd_selection.rs',
    'crates/viboceros-command/tests/support/subdivide.rs',
    'crates/viboceros-command/tests/support/subd_facets.rs',
    'crates/viboceros-geometry/src/mesh/subdivision.rs',
    'crates/viboceros-geometry/src/mesh/subdivision/neighbors.rs',
    'crates/viboceros-geometry/src/mesh/subdivision/ngons.rs',
    'crates/viboceros-geometry/src/subd/subdivision.rs',
    'crates/viboceros-geometry/tests/subdivision.rs',
    'crates/viboceros-geometry/tests/mesh_subdivision_native.rs',
    'crates/viboceros-geometry/tests/support/subdivision_native.rs',
    'src/app/subdivide.rs', 'src/app/subdivide/tests.rs',
    'src/app/component_selection.rs', 'src/app/object_selection.rs',
    'src/viewport.rs', 'src/viewport/display_cache.rs',
    'src/viewport/component_picking.rs', 'src/viewport/picking.rs',
    'src/viewport/parametric_reference.rs', 'src/viewport/polygon_components.rs',
    'src/viewport/polygon_components/tests.rs',
    'tools/rhino_oracle/subdivide_probe.py',
    'tools/rhino_oracle/generate_subdivide.py',
    'tools/rhino_oracle/report_subdivide.py', 'tools/rhino_oracle/test_subdivide.py',
] + [f'tools/rhino_oracle/{directory}/{family}.json'
     for family, _ in FAMILIES + EXPLORATORY for directory in ['fixtures', 'observations']]))
SOURCES = with_shared_sources(SOURCES)


def getter_eligible(value):
    """The viewport picks complete displayed n-gons; typed Faces= uses raw faces."""
    for source in value['input']['sources']:
        selected = set(source.get('selected_faces', []))
        for ngon in source.get('ngons', []):
            members = set(ngon['faces'])
            if selected & members and not members <= selected:
                return False
    return True


def observations():
    expected, engines = {}, set()
    for family, count in FAMILIES:
        request = json.loads((HERE / 'fixtures' / (family + '.json')).read_text())
        native = json.loads((HERE / 'observations' / (family + '.json')).read_text())
        rows = native['results']
        if len(rows) != count or [r['id'] for r in rows] != [o['id'] for o in request['operations']]:
            raise ValueError('missing or reordered native subdivision recipes')
        engines.add(native['engine_version'])
        for row in rows:
            key, value = row['id'], row['value']
            if key in expected or len(value['steps']) != value['input'].get('repeat', 1):
                raise ValueError('duplicate subdivision recipe or missing steps')
            for step in value['steps']:
                events = [e for e in step['events'] if e['name'] == 'SubDivide']
                success = not value['input'].get('cancel', False)
                if len(events) != 1 or step['success'] is not success or events[0]['result'] != ('Success' if success else 'Cancel') or events[0]['objects'] != step['after'] or 'Unknown command:' in step['history']:
                    raise ValueError('invalid native subdivision lifecycle')
                if sorted(o['source'] for o in step['after']) != list(range(len(value['input']['sources']))):
                    raise ValueError('native subdivision source identity differs')
            native_history_error(value['after_script'], value['redo'])
            expected[key] = value
    if len(engines) != 1:
        raise ValueError('mixed native subdivision engines')
    return expected, engines.pop()


def mesh_check(actual, target):
    for name in ['qualified', 'raw_seams_colors_and_regions', 'stored_pieces_correspond', 'triangle_facets_correspond']:
        if actual.get(name) is not True:
            raise ValueError('unqualified subdivision mesh or stored pieces')
    for name, data in [('vertices', target['vertices']), ('faces', target['faces']), ('ngons', target['ngons'])]:
        if type(actual.get(name)) is not int or actual[name] != len(data):
            raise ValueError('subdivision mesh topology differs')
    return error(actual.get('maximum_vertex_error'))


def subd_check(actual, target):
    for name in ['qualified', 'surface_sampled', 'surface_facets_correspond', 'common_facet_triangulation']:
        if actual.get(name) is not True or target.get('valid') is not True:
            raise ValueError('unqualified subdivision control net or sampled facets')
    for name, data in [('vertices', target['points']), ('edges', target['edges']), ('faces', target['faces']), ('compared_surface_faces', target['surface_mesh']['faces'])]:
        if type(actual.get(name)) is not int or actual[name] != len(data):
            raise ValueError('subdivision control net or facet count differs')
    return error(actual.get('maximum_control_or_limit_point_error')), error(actual.get('maximum_sampled_surface_error'))


def summarize(text):
    expected, engine = observations()
    stages, mesh_error, control_error, facet_error = {}, 0., 0., 0.
    for stage in ['COMMAND', 'GETTER']:
        ids = [key for key, v in expected.items() if (not v['input'].get('cancel') if stage == 'COMMAND' else getter_eligible(v))]
        rows = [json.loads(s) for s in re.findall(r'^SUBDIVIDE_' + stage + r' (.+)$', text, re.M)]
        if len(rows) != len(ids) or {r['id'] for r in rows} != set(ids):
            raise ValueError('missing, duplicate or unrelated subdivision ' + stage)
        found = {r['id']: r for r in rows}
        rows = [found[key] for key in ids]
        for row in rows:
            check, target = row['check'], expected[row['id']]['after_script']
            if row.get('grouped_history') is not True or check.get('qualified') is not True or check.get('attributes_selection_and_identity') is not True or len(check['objects']) != len(target):
                raise ValueError('unqualified subdivision history, attributes, identity or selection')
            for actual, obj in zip(check['objects'], target):
                if 'mesh' in obj:
                    mesh_error = max(mesh_error, mesh_check(actual, obj['mesh']))
                else:
                    c, f = subd_check(actual, obj['subd'])
                    control_error, facet_error = max(control_error, c), max(facet_error, f)
        stages[stage.lower()] = rows
    keys = [(key, obj['source'], i) for key, v in expected.items() if not v['input'].get('cancel')
            for i, step in enumerate(v['steps']) for obj in step['after'] if 'mesh' in obj]
    rows = [json.loads(s) for s in re.findall(r'^SUBDIVIDE_MESH_KERNEL (.+)$', text, re.M)]
    if any(type(r.get('source')) is not int or type(r.get('step')) is not int for r in rows):
        raise ValueError('invalid mesh kernel source or step index')
    found = {(r['id'], r['source'], r['step']): r for r in rows}
    if len(rows) != len(keys) or set(found) != set(keys):
        raise ValueError('missing, duplicate or unrelated mesh kernel comparisons')
    for key, source, i in keys:
        row = found[key, source, i]
        target = next(o['mesh'] for o in expected[key]['steps'][i]['after'] if o['source'] == source)
        mesh_error = max(mesh_error, mesh_check(row['check'], target))
    stages['mesh_kernel'] = [found[key] for key in keys]
    return dict(
        model_epsilon=1e-6, captured_workflows=len(expected),
        successful_workflows=sum(not v['input'].get('cancel', False) for v in expected.values()),
        command_cases=len(stages['command']), getter_cases=len(stages['getter']),
        mesh_kernel_comparisons=len(keys), exploratory_workflows=16,
        native_engine_version=engine, full_native_parity=False, performance_qualified=False,
        maximum_mesh_vertex_error=mesh_error, maximum_control_or_limit_point_error=control_error,
        maximum_sampled_surface_error=facet_error,
        maximum_native_history_evaluation_error=max(native_history_error(v['after_script'], v['redo']) for v in expected.values()),
        stages=stages,
        limits=[
            'Fixed public command captures and density-two facet correspondence qualify the recorded cases; arbitrary topology, all native gestures and continuous smooth-surface error remain unqualified.',
            'Native component captures use the public SelectSubObject API. Viewport click/window tests are independent frontend regressions; no physical native mouse-gesture parity is claimed.',
            'The viewport selects a complete displayed mesh n-gon. Six partial raw n-gon recipes are checked through the kernel and typed Faces= operation, and excluded from getter totals.',
            'One subdivision per invocation; numeric repeat counts and DeleteInput are rejected. The diagnostic discovery capture preserves those rejected options.',
            'SubD output is bounded to 100000 control vertices/faces, 400000 edges/corners. Mesh output and aggregate candidates are bounded to 4000000 vertices and 1000000 faces.',
            'Material/texture channels, every tag/nonmanifold configuration and comparative performance are not qualified.',
        ],
        sha256={p: hashlib.sha256((ROOT / p).read_bytes()).hexdigest() for p in SOURCES},
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('logs', nargs='+', type=Path)
    args = parser.parse_args()
    result = summarize('\n'.join(p.read_text() for p in args.logs))
    (ROOT / 'docs/subdivide-qualification.json').write_text(json.dumps(result, indent=2, allow_nan=False) + '\n')


if __name__ == '__main__':
    main()
