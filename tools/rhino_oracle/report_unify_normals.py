"""Qualify normal unification without hiding component-seed differences."""
import argparse
import hashlib
import json
import re
from pathlib import Path
from .unify_normals_cases import CASE_GROUPS, request
from .qualification_sources import with_shared_sources
from .report_subd_box import HERE, ROOT, error

GAPS = {'unify_followup_regions_' + suffix for suffix in ['01', '10', '11', '20', '30', '31']}
FAMILIES = {'unify_normals_' + name: ({} if name == 'sdk' else {name: True}, cases)
            for name, cases in CASE_GROUPS.items()}
SOURCES = with_shared_sources([
    'Cargo.lock', 'src/app.rs', 'src/app/object_selection.rs',
    'src/app/object_selection/unify_normals_tests.rs',
    'crates/viboceros-command/src/lib.rs',
    'crates/viboceros-command/src/mesh_edit/mod.rs',
    'crates/viboceros-command/src/mesh_edit/unify_normals.rs',
    'crates/viboceros-command/src/mesh_edit/replacement.rs',
    'crates/viboceros-command/src/tests/mesh_edit/normals.rs',
    'crates/viboceros-command/tests/unify_normals_native.rs',
    'crates/viboceros-command/tests/unify_normals_command_native.rs',
    'crates/viboceros-command/tests/support/unify_normals.rs',
    'crates/viboceros-command/tests/support/mesh_command.rs',
    'crates/viboceros-command/tests/support/mesh_conversion.rs',
    'crates/viboceros-command/tests/support/crease.rs',
    'crates/viboceros-command/tests/support/subd.rs',
    'crates/viboceros-geometry/tests/support/subdivision_native.rs',
    'crates/viboceros-document/src/lib.rs',
    'crates/viboceros-document/src/history.rs',
    'crates/viboceros-document/src/selection.rs',
    'crates/viboceros-geometry/src/mesh.rs',
    'crates/viboceros-geometry/src/mesh/orientation.rs',
    'crates/viboceros-geometry/src/mesh/normals.rs',
    'crates/viboceros-geometry/src/mesh/ngon.rs',
    'crates/viboceros-io/src/three_dm.rs',
    'crates/viboceros-io/src/three_dm/mesh_ngon.rs',
    'crates/viboceros-oracle/src/lib.rs',
    'crates/viboceros-oracle/tests/unify_normals.rs',
    'third_party/opennurbs/opennurbs_mesh.cpp',
    'third_party/opennurbs/opennurbs_mesh.h',
    'third_party/opennurbs/opennurbs_mesh_ngon.cpp',
    'tools/rhino_oracle/diagnostics/unify_normals/seed-staging-failure.json',
    'tools/rhino_oracle/diagnostics/unify_normals/seed-staging-failure.log',
] + ['tools/rhino_oracle/' + p for p in [
    'unify_normals_cases.py', 'unify_normals_seed_cases.py', 'unify_normals_probe.py',
    'generate_unify_normals.py', 'report_unify_normals.py', 'test_unify_normals.py',
    'mesh_channel_capture.py', 'crease_probe.py', 'crease_cases.py',
    'subdivide_probe.py', 'subd_box_probe.py', 'join_probe.py', 'merge_edges_probe.py',
    'client.py', 'rhino_worker.py', 'structural_sdk.py',
]])


def native():
    versions = set()
    groups = {'sdk': {}, 'command': {}}
    for family, (flags, cases) in FAMILIES.items():
        query = request(**flags)
        data = json.loads((HERE / 'observations' / (family + '.json')).read_text())
        versions.add(data['engine_version'])
        if (json.loads((HERE / 'fixtures' / (family + '.json')).read_text()) != query
                or [r['id'] for r in data['results']] != list(cases)):
            raise ValueError('missing or changed normal unification references')
        for row in data['results']:
            v = row.get('value')
            if v is None or v['input'] != cases[row['id']]:
                raise ValueError('normal unification input recipe differs')
            command = 'command' in v['input']
            if command:
                accepted = not v['input'].get('cancel', False)
                events = [e for e in v['events'] if e['name'] == 'UnifyMeshNormals']
                if (v['success'] is not accepted or len(events) != 1
                        or events[0]['result'] != ('Success' if accepted else 'Cancel')
                        or 'Unknown command' in v['history']):
                    raise ValueError('native normal command admission differs')
            elif (v['before'] != v['after_count'] or v['after'] != v['after_second']
                    or v['predicted'] != v['changed'] or v['second'] != 0
                    or v['after']['valid'] is not True):
                raise ValueError('native count-only or idempotence reference differs')
            groups['command' if command else 'sdk'][row['id']] = v
    if len(versions) != 1:
        raise ValueError('mixed normal unification engines')
    return versions.pop(), groups


def summarize(text):
    version, groups = native()
    stages = {}
    maximum = 0.
    for stage in ['SDK', 'COMMAND', 'GETTER']:
        expected = groups['sdk' if stage == 'SDK' else 'command']
        if stage == 'COMMAND':
            expected = {k: v for k, v in expected.items() if v['success']}
        rows = [json.loads(s) for s in re.findall('^UNIFY_NORMALS_' + stage + r' (.+)$', text, re.M)]
        if len(rows) != len(expected) or {r['id'] for r in rows} != set(expected):
            raise ValueError('missing or duplicate normal unification ' + stage)
        for row in rows:
            v = expected[row['id']]
            gap = stage == 'SDK' and row['id'] in GAPS
            flags = ['qualified', 'geometry', 'channels', 'normals', 'ngons']
            flags += (['count_only', 'source_unchanged', 'idempotent'] if stage == 'SDK'
                      else ['identity', 'metadata', 'selection', 'object_order', 'history'])
            for key in flags:
                wanted = not (gap and key in ['qualified', 'geometry'])
                if row.get(key) is not wanted:
                    raise ValueError('unqualified or relabelled normal unification evidence')
            if stage == 'SDK':
                for key, wanted in [('vertices', len(v['after']['vertices'])),
                                    ('faces', len(v['after']['faces'])),
                                    ('predicted', v['predicted']), ('changed', v['changed'])]:
                    if type(row.get(key)) is not int or row[key] != wanted:
                        raise ValueError('incorrect normal unification counts')
            elif row.get('accepted') is not v['success']:
                raise ValueError('incorrect normal unification acceptance')
            maximum = max(maximum, error(row.get('maximum_error')))
        stages[stage.lower()] = sorted(rows, key=lambda r: list(expected).index(r['id']))
    sources = list(SOURCES)
    for family in FAMILIES:
        sources.extend('tools/rhino_oracle/' + d + '/' + family + '.json'
                       for d in ['fixtures', 'observations'])
        sources.extend(str(p.relative_to(ROOT)) for p in sorted((HERE / 'fixtures' / family).glob('*.3dm')))
    return dict(
        native_engine_version=version, model_epsilon=1e-6,
        sdk_cases=622, qualified_sdk_cases=616, sdk_orientation_gaps=6,
        native_command_workflows=47, registered_command_cases=45, getter_cases=47,
        maximum_error=maximum, full_native_parity=False, performance_qualified=False,
        **stages, limits=[
            'Six hundred twenty-two independent SDK inputs compare raw vertices and face winding, stored normal sign changes, UV/colors, n-gon cycles, read-only counts and idempotence. Six hundred sixteen match exactly. Six three-region inputs select different consistent component orientations and remain unqualified; their zero positional error does not qualify winding parity. The earlier two-triangle vertex-renumbering gap is resolved.',
            'Seed controls cover all 24 raw vertex permutations, all nine pairs of cyclic triangle origins and both face orders (432 inputs), plus 135 controls for raw seams, quad origins, unused aliases and disconnected regions. The local initial seed follows the direction of a face closing edge in welded topology indices. Disconnected component seed ordering remains a compatibility limit.',
            'The SDK path preserves unused vertices, raw coincident seams and n-gon boundary cycles. Stored normals on the union of vertices referenced by flipped faces are negated once. Both overloads and count-only queries are captured. Cached face-normal storage is not represented locally.',
            'Forty-five accepted registered commands and forty-seven application getters compare saved source archives, exact raw channels and geometry, identity, attributes/groups, selection, object order and Undo/Redo. Changed meshes use public OpenNURBS OrientNgons and normal reconstruction without rotating n-gon origins or rebuilding face membership; unused vertices survive. Source document admission already aligns n-gon cycles. Non-mesh selections are skipped.',
            'Only changed meshes move to the table end and record geometry history. Preselected changes remain selected on acceptance and Undo; Redo releases them. Unchanged and ignored peers release selection on history replay. All-no-op acceptance retains current picks without creating an Undo step, including command-first picks. Cancel abandons command-first selection without geometry edits.',
            'The compatibility graph follows the first incident face at nonmanifold edges and accepts a best-effort spanning orientation for contradictory cycles. The separate strict constructor operation still skips nonmanifold constraints and rejects nonorientable manifold constraints. Arbitrary component seeds, additional nonorientable/nonmanifold meshes, invalid n-gon repair, restricted objects, grips, physical native postselection, numeric extremes and comparative performance remain unqualified.',
            'One failed seed capture omitted a staged helper and stopped on import at operation zero. Its frozen producer manifest and log are diagnostic evidence and qualify no geometry. The dependency staging check passed before the subsequent successful capture.',
        ], sha256={p: hashlib.sha256((ROOT / p).read_bytes()).hexdigest()
                   for p in dict.fromkeys(sources)})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('logs', type=Path, nargs='+')
    args = parser.parse_args()
    data = summarize('\n'.join(p.read_text() for p in args.logs))
    (ROOT / 'docs/unify-normals-qualification.json').write_text(
        json.dumps(data, indent=2, allow_nan=False) + '\n')


if __name__ == '__main__':
    main()
