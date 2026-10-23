"""Qualify the SubD insertion core; retain unreplayed command and mesh captures."""
import argparse
import hashlib
import json
import re
from pathlib import Path
from .qualification_sources import with_shared_sources
from .report_subd_box import HERE, ROOT, error

FAMILIES = {'insert_point_discovery': 4, 'insert_point_live': 8,
            'insert_point_full': 21, 'insert_point_regions': 12}
SOURCES = with_shared_sources([
    'Cargo.lock', 'crates/viboceros-opennurbs/build.rs',
    'crates/viboceros-opennurbs/native/subd.cpp',
    'crates/viboceros-opennurbs/native/subd.h',
    'crates/viboceros-opennurbs/native/subd_codec.h',
    'crates/viboceros-opennurbs/src/lib.rs',
    'crates/viboceros-opennurbs/src/subd.rs',
    'crates/viboceros-geometry/src/lib.rs',
    'crates/viboceros-geometry/src/subd.rs',
    'crates/viboceros-geometry/tests/subd_edge_insert.rs',
    'crates/viboceros-command/src/lib.rs',
    'crates/viboceros-command/tests/insert_point_native.rs',
    'crates/viboceros-command/tests/support/crease.rs',
    'crates/viboceros-command/tests/support/subd.rs',
    'third_party/opennurbs/opennurbs_subd.cpp',
    'third_party/opennurbs/opennurbs_subd.h',
] + ['tools/rhino_oracle/' + name for name in [
    'insert_point_cases.py', 'insert_point_probe.py', 'generate_insert_point.py',
    'conic_getter_input.py', 'managed_mouse_callback.py', 'snap_environment.py',
    'number_token.py', 'catenary_point_picker.py', 'group_picking.py',
    'crease_probe.py', 'crease_cases.py', 'subdivide_probe.py', 'subd_box_probe.py',
    'join_probe.py', 'merge_edges_probe.py', 'client.py', 'rhino_worker.py',
    'structural_sdk.py', 'report_insert_point.py', 'test_insert_point.py',
]])


def observations():
    expected, excluded, engines = {}, {}, set()
    for family, count in FAMILIES.items():
        query = json.loads((HERE/'fixtures'/(family+'.json')).read_text())
        capture = json.loads((HERE/'observations'/(family+'.json')).read_text())
        engines.add(capture['engine_version'])
        if (len(capture['results']) != count or
                [r['id'] for r in capture['results']] != [r['id'] for r in query['operations']]):
            raise ValueError('missing or reordered insertion captures')
        for row in capture['results']:
            key = row['id']
            value = row.get('value')
            if value is None:
                excluded[key] = 'Producer error retained unchanged.'
            elif family == 'insert_point_discovery':
                excluded[key] = 'Typed-coordinate discovery did not provide reliable native edge-pick references.'
            elif family == 'insert_point_live':
                excluded[key] = 'Early physical capture predates independent ray and projected-point witnesses.'
            elif any(t.get('errors') or t.get('completed') != t.get('expected') for t in value['input_trace']):
                excluded[key] = 'Incomplete native physical input; retained as a diagnostic.'
            elif not key.startswith('insert_point_full_subd_'):
                excluded[key] = 'Outside this SubD-only kernel record; mesh command comparisons are in insert-point-command-qualification.json.'
            else:
                if 'Unknown command' in value['history']:
                    raise ValueError('unexpected native insertion token')
                clicks=value['input_trace'][0].get('clicks', [])
                if len(clicks)!=sum(isinstance(a,dict) for a in value['input']['actions']):
                    raise ValueError('missing native physical point witnesses')
                for click in clicks:
                    if not click.get('click_acknowledged') or len(click.get('ray', [])) != 2 or len(click.get('projected_point', [])) != 3:
                        raise ValueError('missing native physical point witness')
                expected[key] = value
    if len(expected) != 10 or len(engines) != 1:
        raise ValueError('unexpected insertion corpus')
    return expected, excluded, engines.pop()


def source_hashes():
    paths = list(SOURCES)
    for family in FAMILIES:
        paths.extend('tools/rhino_oracle/'+folder+'/'+family+'.json' for folder in ['fixtures', 'observations'])
        paths.extend(str(p.relative_to(ROOT)) for p in sorted((HERE/'fixtures'/family).glob('*.3dm')))
    return {p: hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in dict.fromkeys(paths)}


def summarize(text):
    expected, excluded, engine = observations()
    rows = [json.loads(s) for s in re.findall(r'^INSERT_POINT_KERNEL (.+)$', text, re.M)]
    if len(rows) != len(expected) or {r['id'] for r in rows} != set(expected):
        raise ValueError('missing, duplicate or unrelated insertion replay')
    maximum_control = maximum_surface = 0.
    for row in rows:
        target = next(o['subd'] for o in expected[row['id']]['after'] if o['source'] == 0)
        actual = row.get('comparison', {})
        if target['valid'] is not True or actual.get('qualified') is not True or actual.get('surface_sampled') is not True:
            raise ValueError('unqualified SubD insertion geometry')
        for key, data in [('vertices', target['points']), ('edges', target['edges']), ('faces', target['faces'])]:
            if type(actual.get(key)) is not int or actual[key] != len(data):
                raise ValueError('SubD insertion topology differs')
        maximum_control = max(maximum_control, error(actual.get('maximum_control_or_limit_point_error')))
        maximum_surface = max(maximum_surface, error(actual.get('maximum_sampled_surface_error')))
    rows.sort(key=lambda r: list(expected).index(r['id']))
    return dict(native_engine_version=engine, model_epsilon=1e-6,
                captured_workflows=sum(FAMILIES.values()), subd_kernel_cases=len(rows),
                maximum_control_or_limit_point_error=maximum_control,
                maximum_sampled_surface_error=maximum_surface,
                cases=rows, excluded=excluded, command_implemented=True,
                command_qualification_record='insert-point-command-qualification.json',
                full_native_parity=False, performance_qualified=False,
                limits=[
                    'This record qualifies only the SubD path kernel. The registered command, interactive getter, document history, local Undo and cancellation have separate evidence in insert-point-command-qualification.json. Grip-state parity remains unqualified.',
                    'The ten replays use exact saved source archives and captured physical projected-point witnesses in an owned Top viewport. Control-point ordinals, oriented faces, edge and vertex tags, limit stations and density-two facets agree within epsilon.',
                    'The path core uses public OpenNURBS SplitEdge and face-local-index SplitFace in one archive decode/edit/encode. Explicit edge points precede automatic crossings through intermediate faces. Near-endpoint user picks clamp to [0.001,0.999].',
                    'Mesh captures, early unwitnessed physical input, typed-coordinate discovery and one incomplete physical input remain excluded from this kernel record. Separate command evidence retains mesh channels, logical regions and explicit native tessellation differences.',
                    'The core accepts at most 256 original-edge picks and one million source point-plus-edge components. A supplied parallel-view normal controls crossing projection; otherwise the first incident face supplies its normal. Perspective views, intersecting path chords, arbitrary 3D topology, continuous surface error and comparative speed remain unqualified.',
                ], sha256=source_hashes())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('logs', nargs='+', type=Path)
    args = parser.parse_args()
    report = summarize('\n'.join(p.read_text() for p in args.logs))
    (ROOT/'docs/insert-point-kernel-qualification.json').write_text(json.dumps(report, indent=2, allow_nan=False)+'\n')


if __name__ == '__main__':
    main()
