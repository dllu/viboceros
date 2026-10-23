"""Qualify normal unification without hiding component-seed differences."""
import argparse
import hashlib
import json
import re
from pathlib import Path
from .unify_normals_cases import CASE_GROUPS, request
from .qualification_sources import with_shared_sources
from .report_subd_box import HERE, ROOT, error
from .unify_normals_evidence import check_sdk_row, raw

GAPS = {'unify_component_' + name for name in [
    'nonmanifold_01', 'nonmanifold_11', 'nonmanifold_21', 'tetra_201', 'tetra_211', 'tetra_221',
    'mobius_011', 'mobius_111', 'mobius_211',
]} | {'unify_runtime_tetra_201', 'unify_runtime_nonmanifold_01'}
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
    'unify_normals_cases.py', 'unify_normals_seed_cases.py', 'unify_normals_walk_cases.py',
    'unify_normals_probe.py', 'unify_normals_evidence.py',
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
            else:
                if (v['before'] != v['after_count'] or v['predicted'] != v['changed']
                        or v['after']['valid'] is not True or v['after_second']['valid'] is not True
                        or any(v[k]['vertices'] != v['before']['vertices'] for k in ['after', 'after_second'])):
                    raise ValueError('native count-only or storage reference differs')
                if v['input'].get('runtime_checks'):
                    for trial in v['runtime'].values():
                        if (raw(trial['before']) != raw(v['after'])
                                or raw(trial['after']) != raw(v['after_second'])
                                or trial['changed'] != v['second']):
                            raise ValueError('fresh mesh runtime reference differs')
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
            if stage == 'SDK':
                check_sdk_row(row, v)
                if row['qualified'] is not (row['id'] not in GAPS):
                    raise ValueError('SDK winding differences changed')
            else:
                flags = ['qualified', 'geometry', 'channels', 'normals', 'ngons', 'identity',
                         'metadata', 'selection', 'object_order', 'history']
                if any(row.get(key) is not True for key in flags) or row.get('accepted') is not v['success']:
                    raise ValueError('incorrect normal unification command evidence')
            maximum = max(maximum, error(row.get('maximum_error')))
        stages[stage.lower()] = sorted(rows, key=lambda r: list(expected).index(r['id']))
    sources = list(SOURCES)
    for family in FAMILIES:
        sources.extend('tools/rhino_oracle/' + d + '/' + family + '.json'
                       for d in ['fixtures', 'observations'])
        sources.extend(str(p.relative_to(ROOT)) for p in sorted((HERE / 'fixtures' / family).glob('*.3dm')))
    return dict(
        native_engine_version=version, model_epsilon=1e-6,
        sdk_cases=len(stages['sdk']), qualified_sdk_cases=sum(r['qualified'] for r in stages['sdk']),
        sdk_orientation_gaps=len(GAPS), native_repeat_edit_cases=sum(v['second'] != 0 for v in groups['sdk'].values()),
        native_command_workflows=len(groups['command']), registered_command_cases=len(stages['command']),
        getter_cases=len(stages['getter']),
        maximum_error=maximum, full_native_parity=False, performance_qualified=False,
        **stages, limits=[
            'There are 1180 recorded SDK workflows: 1169 match raw geometry, channels, face counts and repeat calls exactly. Eleven workflows remain unqualified: nine secondary-component inputs and two runtime controls for those inputs. The earlier six three-region differences and the two-triangle renumbering gap are resolved. Zero positional error alone does not qualify winding parity.',
            'Every SDK row records first-call and repeat face-index edits. An independent permutation checker reconstructs winding and the union of affected raw normal vertices from those edits, then derives geometry, channel, count, repeat and qualification flags against native snapshots. It does not duplicate the traversal algorithm.',
            'Controls include 432 raw-index/cyclic-origin permutations, 135 seam/quad/alias inputs, 24 mixed-region inputs, 480 systematic consistent/inconsistent region combinations and 48 structural inputs involving isolated faces, vertex contacts, closed tetrahedra, nonmanifold fans and twisted strips. Traversal follows face-local sides, beginning with the closing side. Disconnected regions share an initial orientation choice.',
            'Eight native workflows perform repeat edits on isolated faces; matching those workflows does not imply idempotence. Six runtime controls compare duplicates and freshly rebuilt meshes, with and without count-only queries: their repeat geometry and counts agree with the original instances. This evidence does not cover other cached state. Cached face-normal storage is not represented locally.',
            'The SDK path preserves unused vertices, raw coincident seams and n-gon boundary cycles. Stored normals on the union of vertices referenced by flipped faces are negated once. Both overloads and read-only counts are captured.',
            'Sixty-three accepted registered commands and sixty-five application getters compare source archives, raw channels and geometry, identity, attributes/groups, selection, order and Undo/Redo. Changed meshes use public OpenNURBS OrientNgons and normal reconstruction without rotating n-gon origins or rebuilding membership; unused vertices survive. Native document admission already aligns n-gon cycles. Non-mesh selections are skipped.',
            'Only changed meshes move to the table end and record geometry history. Preselection survives acceptance and Undo; Redo releases it. Unchanged and ignored peers release selection on replay. All-no-op acceptance retains current picks without a new Undo step, including command-first picks. Cancel abandons picking without geometry edits.',
            'Secondary closed, nonmanifold and twisted component seed choices still differ in eleven workflows. The compatibility graph uses first-incident-face constraints at nonmanifold edges and a best-effort spanning orientation for contradictory cycles. The strict constructor operation still skips nonmanifold edges and rejects nonorientable manifold constraints. Broader seed ordering, invalid n-gon repair, restricted objects, grips, physical native postselection, numeric extremes and performance remain unqualified.',
            'One earlier seed capture omitted a staged helper and stopped on import at operation zero. Its frozen manifest and log qualify no geometry. Later captures passed standalone staging checks.',
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
