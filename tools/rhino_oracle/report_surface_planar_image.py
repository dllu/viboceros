"""Audit complete planar image proofs and physical FitSrf capture outcomes."""
import json
from .report_surface_fit_basis import ROOT, digest, read

PROOF = ROOT / 'docs/surface-planar-image-proof'
FIT_FAMILIES = ['surface_fit', 'surface_fit_adaptive', 'surface_fit_strips']
COMMAND_FAMILIES = ['surface_fit_commands', 'surface_fit_command_faces']
SOURCE_OVERRIDES = [
    'crates/viboceros-geometry/src/surface_pullback/certificate/' + name for name in
    ['surface.rs', 'surface_pair.rs', 'surface_pair/planar_image.rs',
     'surface_pair/planar_image/half_space.rs', 'surface_pair/planar_image/tests.rs']
] + ['crates/viboceros-oracle/' + name for name in
     ['src/lib.rs', 'src/surface_planar_image.rs', 'tests/surface_planar_image_native.rs']] + [
    'tools/rhino_oracle/' + name for name in
    ['rhino_worker.py', 'structural_sdk.py', 'surface_fit_command_probe.py',
     'generate_surface_fit_commands.py', 'generate_surface_fit_command_faces.py',
     'test_surface_fit_commands.py', 'report_surface_planar_image.py',
     'test_surface_planar_image_record.py', 'run_headless.sh']
]


def rows(log, prefix):
    return [json.loads(line[len(prefix):]) for line in log.splitlines() if line.startswith(prefix)]


def summarize_images(captures, log):
    expected = [r for capture in captures for r in capture['results'] if r['value']['surface'] is not None]
    observed = rows(log, 'PLANAR_FIT_IMAGE ')
    assert len(expected) == len(observed) == 90
    assert len({r['id'] for r in expected}) == 90
    assert [r['id'] for r in expected] == [r['id'] for r in observed]
    certified = parameter_unproved = 0
    maximum = 0.
    for native, row in zip(expected, observed):
        value = native['value']
        assert value['before'] == value['after']
        flat = all(p['point'][2] == 0. for definition in [value['before'], value['surface']]
                   for p in definition['control_points'])
        assert row['planar_fixture'] is flat
        bound = row['image_bound']
        if flat:
            assert type(bound) in (int, float) and 0. <= bound <= 1e-9
            maximum = max(maximum, bound)
            certified += 1
            if row['parameter_bound'] is None:
                parameter_unproved += 1
            else:
                assert 0. <= row['parameter_bound'] <= 1e-9
        else:
            assert bound is None and row['parameter_bound'] is None
    assert (certified, parameter_unproved) == (30, 22)
    assert rows(log, 'PLANAR_FIT_IMAGE_TOTAL ') == [dict(cases=90, certified_planar_images=30, parameter_unproved=22)]
    return dict(source_native_pairs=90, certified_planar_images=30, parameter_unproved=22,
                image_unproved=60, maximum_image_bound=maximum)


def summarize_commands(queries, captures, log):
    observed = rows(log, 'PHYSICAL_SURFACE_FIT ')
    expected = [(op, r) for query, capture in zip(queries, captures)
                for op, r in zip(query['operations'], capture['results'])]
    assert [len(q['operations']) for q in queries] == [8, 7]
    assert [len(c['results']) for c in captures] == [8, 7]
    assert len(observed) == len(expected) == len({r['id'] for r in observed}) == 15
    counts = dict(accepted=0, selection_rejected=0, fit_declined=0)
    for (op, native), row in zip(expected, observed):
        assert op['id'] == native['id'] == row['id']
        value = native['value']; command = value['command']
        assert command['completed'] == 1 and command['active'] is False
        assert command['after'] == command['after_cancel']
        assert value['undo']['success'] is True and value['redo']['success'] is True
        assert value['undo']['after'] == value['before']
        assert value['redo']['after'] == command['after']
        faces = len(value['before'][0]['definitions'])
        reason = 'selection_rejected' if faces > 1 else 'fit_declined' if value['sdk'] is None else 'accepted'
        accepted = reason == 'accepted'
        assert row['reason'] == reason and row['input_faces'] == faces
        assert row['accepted'] is command['success'] is accepted
        assert row['undo_redo'] is True
        if accepted:
            assert row['sdk_bound'] == 0.
            target = command['after'][0 if op['delete_input'] else 1]
            assert target['definition'] == value['sdk']
        else:
            assert row['sdk_bound'] is None
            assert command['after'] == value['before']
            if reason == 'fit_declined':
                assert 'could not be simplified' in command['history']
        counts[reason] += 1
    assert counts == dict(accepted=9, selection_rejected=3, fit_declined=3)
    return dict(**counts, sdk_equal_refits=9, undo_redo_workflows=15)


def audit_producer(capture, query_bytes):
    assert capture['provenance']['request_sha256'] == digest(query_bytes)
    for sha in capture['provenance']['sources'].values():
        path = ROOT / ('tools/rhino_oracle/observations/source_snapshots/' + sha + '.gz')
        assert digest(read(path)) == sha


def report():
    receipt = json.loads((PROOF / 'receipts.json').read_bytes())
    assert set(receipt['source_overrides']) == set(SOURCE_OVERRIDES)
    for name, entry in receipt['source_overrides'].items():
        assert digest(read(ROOT / entry['snapshot'])) == entry['sha256'], name
    required = {item['log'] for item in receipt['checks'].values()}
    required.update(item['log'] for item in receipt['native_launches'])
    required.update(item['capsule'] for item in receipt['native_launches'] if item['exit_code'])
    required.update(item['log'] for item in receipt['rejected_runs'])
    captures = {}; queries = {}; query_bytes = {}
    for family in FIT_FAMILIES + COMMAND_FAMILIES:
        fixture = 'tools/rhino_oracle/fixtures/' + family + '.json'
        observation = 'tools/rhino_oracle/observations/' + family + '.json.gz'
        required.update([fixture, observation])
        query_bytes[family] = (ROOT / fixture).read_bytes()
        queries[family] = json.loads(query_bytes[family])
        captures[family] = json.loads(read(ROOT / observation))
        audit_producer(captures[family], query_bytes[family])
        assert [o['id'] for o in queries[family]['operations']] == [r['id'] for r in captures[family]['results']]
    assert required.issubset(receipt['files'])
    for name, sha in receipt['files'].items():
        assert digest((ROOT / name).read_bytes()) == sha, name
    checks = receipt['checks']
    assert set(checks) == {'portable', 'native', 'certificates', 'python', 'app', 'format'}
    assert all(check['exit_code'] == 0 for check in checks.values())
    assert 'test result: ok. 103 passed; 0 failed;' in read(ROOT / checks['certificates']['log']).decode()
    assert 'Ran 3 tests' in read(ROOT / checks['python']['log']).decode()
    launches = receipt['native_launches']
    assert [item['exit_code'] for item in launches] == [1, 1, 0, 0]
    assert len(receipt['rejected_runs']) == 2
    for rejected in receipt['rejected_runs']:
        assert rejected['exit_code'] == 101
        assert 'test result: FAILED.' in read(ROOT / rejected['log']).decode()
    for launch in launches:
        assert launch['command'].startswith('tools/rhino_oracle/run_headless.sh exec ')
        if launch['exit_code']:
            failure = json.loads(read(ROOT / launch['capsule']))
            assert 'unexpected FitSrf' in failure['error']
            audit_producer(failure, query_bytes['surface_fit_commands'])
        else:
            assert 'Recorded ' + launch['family'] in read(ROOT / launch['log']).decode()
    backends = {}
    for backend in ['portable', 'native']:
        log = read(ROOT / checks[backend]['log']).decode()
        assert 'test result: ok. 3 passed; 0 failed;' in log
        backends[backend] = dict(
            images=summarize_images([captures[f] for f in FIT_FAMILIES], log),
            commands=summarize_commands([queries[f] for f in COMMAND_FAMILIES],
                                       [captures[f] for f in COMMAND_FAMILIES], log))
    return dict(full_rhino_fit_parity=False, fit_srf_registered=False,
                automatic_fit_admission_changed=False, normalized_uv_contract_changed=False,
                backends=backends, baseline_revision=receipt['baseline_revision'],
                receipts='surface-planar-image-proof/receipts.json')


if __name__ == '__main__':
    print(json.dumps(report(), indent=2, allow_nan=False))
