"""Qualify fixed Crease/RemoveCrease comparisons, rejecting incomplete evidence."""
from .qualification_sources import with_application_sources
import argparse
import copy
import hashlib
import json
import re
from pathlib import Path

from .report_subd_box import HERE, ROOT, error
from .report_subd_ellipsoid import native_history_error

FAMILIES = [('crease_main_v3', 62, 54), ('crease_extra', 30, 30),
            ('crease_policy', 12, 12), ('crease_mixed', 10, 10),
            ('crease_overlap', 4, 4)]
DIAGNOSTICS = ['crease_discovery', 'crease_main_export_diagnostic',
               'crease_main', 'crease_main_v2']
SOURCES = [
    'crates/viboceros-opennurbs/build.rs',
    'crates/viboceros-opennurbs/native/subd.cpp',
    'crates/viboceros-opennurbs/native/subd.h',
    'crates/viboceros-opennurbs/native/subd_codec.h',
    'crates/viboceros-opennurbs/native/subd_crease.h',
    'crates/viboceros-opennurbs/native/subd_edges.h',
    'crates/viboceros-opennurbs/src/lib.rs',
    'crates/viboceros-opennurbs/src/subd.rs',
    'crates/viboceros-opennurbs/src/subd/crease.rs',
    'crates/viboceros-opennurbs/src/subd/edges.rs',
    'crates/viboceros-geometry/src/subd.rs',
    'crates/viboceros-geometry/src/subd/crease.rs',
    'crates/viboceros-geometry/src/subd/edges.rs',
    'crates/viboceros-geometry/tests/subd_edges.rs',
    'crates/viboceros-geometry/src/mesh.rs',
    'crates/viboceros-geometry/src/mesh/edge_weld.rs',
    'crates/viboceros-document/src/object_geometry.rs',
    'crates/viboceros-document/src/control_points.rs',
    'crates/viboceros-command/src/lib.rs',
    'crates/viboceros-command/src/crease.rs',
    'crates/viboceros-command/src/crease/arguments.rs',
    'crates/viboceros-command/tests/crease_native.rs',
    'crates/viboceros-command/tests/crease_staging.rs',
    'crates/viboceros-command/src/component_selection.rs',
    'crates/viboceros-command/tests/support/crease.rs',
    'crates/viboceros-command/tests/support/subd.rs',
    'crates/viboceros-command/tests/support/subd_facets.rs',
    'crates/viboceros-geometry/tests/support/subdivision_native.rs',
    'tools/rhino_oracle/crease_cases.py',
    'tools/rhino_oracle/crease_probe.py',
    'tools/rhino_oracle/generate_crease.py',
    'tools/rhino_oracle/report_crease.py',
    'tools/rhino_oracle/test_crease.py',
    'tools/rhino_oracle/structural_sdk.py',
    'tools/rhino_oracle/rhino_worker.py',
    'src/app.rs', 'src/app/crease.rs', 'src/app/crease/tests.rs',
    'src/app/crease/tests/native.rs', 'src/app/component_selection.rs',
    'src/app/command_line.rs', 'src/app/object_selection.rs',
    'src/app/untrim_holes.rs', 'src/app/block_edit.rs',
    'src/viewport.rs', 'src/viewport/component_picking.rs',
    'src/viewport/display_cache.rs', 'src/viewport/polygon_edges.rs',
    'src/viewport/polygon_edges/tests.rs', 'src/viewport/selection.rs',
    'src/viewport/picking.rs', 'src/viewport/screen.rs',
]
SOURCES = with_application_sources(SOURCES)


def observations():
    expected, engines = {}, set()
    for family, count, successful in FAMILIES:
        query = json.loads((HERE / 'fixtures' / (family + '.json')).read_text())
        native = json.loads((HERE / 'observations' / (family + '.json')).read_text())
        rows = native['results']
        if len(rows) != count or [r['id'] for r in rows] != [o['id'] for o in query['operations']]:
            raise ValueError('missing or reordered native crease recipes')
        engines.add(native['engine_version'])
        qualified = 0
        for row in rows:
            key, value = row['id'], row['value']
            events = [e for e in value['events'] if e['name'] == value['input']['command']]
            success = not value['input'].get('postselect', False)
            if (len(events) != 1 or value['success'] is not success
                    or events[0]['result'] != ('Success' if success else 'Cancel')
                    or events[0]['objects'] != value['after']
                    or 'Unknown command:' in value['history']):
                raise ValueError('invalid native crease lifecycle')
            if not success:
                continue
            if key in expected:
                raise ValueError('duplicate native crease recipe')
            completed = copy.deepcopy(value['after'])
            cleaned = copy.deepcopy(value['after_script'])
            for stage in [completed, cleaned]:
                for obj in stage:
                    for name in ['grips_on', 'selected_grips', 'selected_edges', 'selected_vertices']:
                        obj.pop(name, None)
            native_history_error(completed, cleaned)
            native_history_error(value['after_script'], value['redo'])
            expected[key] = value
            qualified += 1
        if qualified != successful:
            raise ValueError('unexpected successful crease workflow count')
    if len(engines) != 1:
        raise ValueError('mixed native crease engines')
    return expected, engines.pop()


def processed_objects(value):
    before = {o['source']: o for o in value['before'] if o['source'] is not None}
    return sum('mesh' in o or o['crc'] != before[o['source']]['crc']
               for o in value['after'] if o['source'] is not None)


def empty_ngon_slots(value):
    return sum(not n['vertices'] and not n['faces'] for o in value['after']
               for n in o.get('mesh', {}).get('ngons', []))


def source_hashes():
    paths = SOURCES + [f'tools/rhino_oracle/{directory}/{family}.json'
                      for family in [f for f, _, _ in FAMILIES] + DIAGNOSTICS
                      for directory in ['fixtures', 'observations']]
    for family, _, _ in FAMILIES:
        paths.extend(str(p.relative_to(ROOT)) for p in sorted((HERE / 'fixtures' / family).glob('*.3dm')))
    return {p: hashlib.sha256((ROOT / p).read_bytes()).hexdigest() for p in paths}


def summarize(text):
    expected, engine = observations()
    rows = [json.loads(s) for s in re.findall(r'^CREASE_COMMAND (.+)$', text, re.M)]
    if len(rows) != len(expected) or {r['id'] for r in rows} != set(expected):
        raise ValueError('missing, duplicate or unrelated crease comparisons')
    found = {r['id']: r for r in rows}
    rows = [found[key] for key in expected]
    maximum = 0.
    for row in rows:
        value = expected[row['id']]
        if any(row.get(name) is not True for name in ['identity', 'attributes', 'history']):
            raise ValueError('unqualified crease identity, attributes or history')
        for name, target in [('processed_objects', processed_objects(value)),
                             ('native_empty_ngon_slots', empty_ngon_slots(value))]:
            if type(row.get(name)) is not int or row[name] != target:
                raise ValueError('incorrect crease ' + name)
        if bool(row['processed_objects']) != any(o['name'] == 'Undo witness' for o in value['undo']):
            raise ValueError('native crease undo witness disagrees with edit count')
        maximum = max(maximum, error(row.get('maximum_error')))
    getters, edge_error = {}, 0.
    getter_rows = [json.loads(s) for s in re.findall(r'^CREASE_GETTER (.+)$', text, re.M)]
    keys = {(phase, key) for phase in ['preselected', 'postselected'] for key in expected}
    found_getters = {(r['phase'], r['id']): r for r in getter_rows}
    if len(getter_rows) != len(keys) or set(found_getters) != keys:
        raise ValueError('missing, duplicate or unrelated crease getter comparisons')
    for phase in ['preselected', 'postselected']:
        getters[phase] = [found_getters[phase, key] for key in expected]
        for row in getters[phase]:
            if any(row.get(name) is not True for name in ['identity', 'attributes', 'history', 'selection']):
                raise ValueError('unqualified crease getter identity, attributes, history or selection')
            maximum = max(maximum, error(row.get('maximum_error')))
            edge_error = max(edge_error, error(row.get('maximum_edge_sample_error')))
    return dict(
        model_epsilon=1e-6, command_cases=len(rows),
        getter_cases=len(getter_rows), getters=getters, maximum_edge_sample_error=edge_error,
        captured_workflows=sum(count for _, count, _ in FAMILIES),
        family_cases={family: count for family, _, count in FAMILIES},
        native_engine_version=engine, maximum_error=maximum,
        full_native_parity=False, performance_qualified=False, cases=rows,
        limits=[
            'The command API, typed helpers, control grips and viewport mesh/SubD edge and SubD vertex picking are implemented. Getter replay checks 110 workflows both before and after command start against saved native geometry, component selection, attributes and Undo/Redo.',
            'SubD edge samples follow original edge IDs through public OpenNURBS subdivision. Density-two stations and segments correspond to captured native surface-mesh edges; polyline interpolation is a display approximation without a continuous smooth-curve error certificate.',
            'Fixed public command captures qualify control topology, tags, limit stations, density-two surface facets, mesh sharing, colors, n-gons, object identity, attributes, order and Undo/Redo. Arbitrary topology and continuous limit-surface error remain unqualified.',
            'Mesh comparison permits a bijection of raw vertex indices and cyclic face origins. Native empty deleted n-gon slots are omitted from the comparison view; raw captures retain them. RhinoCommon opacity is converted to the model transparency convention.',
            'Rhino 8.32 leaves soft weights unchanged when RemoveCrease targets an already-smooth vertex; explicit soft edges are relaxed. Later online help describes different vertex behavior.',
            'Eight unsuccessful SDK postselection attempts and thirty discovery workflows are diagnostics, outside the 110 successful comparisons. Three source-export failures are retained and excluded.',
            'The eight unsuccessful native postselection attempts remain excluded. Positive postselection getter replay is an independent Viboceros frontend regression against the successful native geometry recipes, not native physical mouse-gesture parity. Mesh normal arrays, material/texture channels and comparative performance are unqualified.',
        ], sha256=source_hashes(),
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('logs', nargs='+', type=Path)
    args = parser.parse_args()
    result = summarize('\n'.join(p.read_text() for p in args.logs))
    (ROOT / 'docs/crease-qualification.json').write_text(json.dumps(result, indent=2, allow_nan=False) + '\n')


if __name__ == '__main__':
    main()
