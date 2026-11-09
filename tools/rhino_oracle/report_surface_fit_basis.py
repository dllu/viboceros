"""Audit frozen Fit observations and explicit-basis replay receipts."""
import gzip
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PROOF = ROOT / 'docs/surface-fit-basis-proof'
SOURCE_OVERRIDES = [
    'crates/viboceros-geometry/src/' + name for name in ['lib.rs', 'surface_fit.rs', 'surface_fit/tests.rs']
] + [
    'crates/viboceros-smlib/src/' + name for name in
    ['lib.rs', 'native.rs', 'native/knot_refinement.rs', 'native/knot_refinement/interpolation.rs']
] + ['crates/viboceros-oracle/tests/surface_fit_basis_native.rs'] + [
    'tools/rhino_oracle/' + name for name in
    ['structural_sdk.py', 'rhino_worker.py', 'surface_fit_probe.py', 'generate_surface_fit.py',
     'test_surface_fit.py', 'report_surface_fit_basis.py', 'test_surface_fit_record.py', 'run_headless.sh']
]


def digest(data):
    return hashlib.sha256(data).hexdigest()


def read(path):
    data = path.read_bytes()
    return gzip.decompress(data) if path.suffix == '.gz' else data


def summarize(query, native, log):
    operations = query['operations']
    expected = native['results']
    rows = [json.loads(line.split(' ', 1)[1]) for line in log.splitlines()
            if line.startswith('SURFACE_FIT_BASIS ')]
    totals = [json.loads(line.split(' ', 1)[1]) for line in log.splitlines()
              if line.startswith('SURFACE_FIT_BASIS_TOTAL ')]
    ids = [op['id'] for op in operations]
    assert len(ids) == len(set(ids)) == 36
    assert [row['id'] for row in expected] == ids == [row['id'] for row in rows]
    assert 'test result: ok. 1 passed; 0 failed;' in log
    counts = dict(captured_bases=0, native_null=0, source_parameter_bounds=0, source_parameter_declines=0)
    maximum_native = maximum_archive = 0.
    for op, captured, row in zip(operations, expected, rows):
        value = captured['value']
        assert value['before'] == value['after']
        assert row['native_null'] is (value['surface'] is None)
        if row['native_null']:
            counts['native_null'] += 1
            assert set(row) == {'id', 'native_null'}
            continue
        counts['captured_bases'] += 1
        for key in ['maximum_native_bound', 'maximum_archive_bound']:
            assert type(row[key]) in (int, float) and 0. <= row[key] <= 1e-9
        maximum_native = max(maximum_native, row['maximum_native_bound'])
        maximum_archive = max(maximum_archive, row['maximum_archive_bound'])
        assert row['requested_fit_tolerance'] == op['fit_tolerance']
        bound = row['source_parameter_bound']
        if bound is None:
            counts['source_parameter_declines'] += 1
        else:
            assert 0. <= bound <= op['fit_tolerance']
            counts['source_parameter_bounds'] += 1
    assert (counts['captured_bases'], counts['native_null']) == (23, 13)
    assert totals == [dict(matched_bases=23, native_null=13, adaptive_fit_qualified=False)]
    return dict(**counts, maximum_native_bound=maximum_native, maximum_archive_bound=maximum_archive)


def report():
    receipt = json.loads((PROOF / 'receipts.json').read_text())
    assert set(receipt['source_overrides']) == set(SOURCE_OVERRIDES)
    required_files = {'tools/rhino_oracle/fixtures/surface_fit.json',
                      'tools/rhino_oracle/observations/surface_fit.json.gz'}
    required_files.update(item['log'] for item in receipt['checks'].values())
    required_files.update(item['log'] for item in receipt['rejected_runs'])
    assert required_files.issubset(receipt['files'])
    for name, sha in receipt['files'].items():
        assert digest((ROOT / name).read_bytes()) == sha, name
    for name, entry in receipt['source_overrides'].items():
        assert digest(read(ROOT / entry['snapshot'])) == entry['sha256'], name
    query_path = ROOT / 'tools/rhino_oracle/fixtures/surface_fit.json'
    native = json.loads(read(ROOT / 'tools/rhino_oracle/observations/surface_fit.json.gz'))
    query = json.loads(query_path.read_bytes())
    assert native['provenance']['request_sha256'] == digest(query_path.read_bytes())
    for sha in native['provenance']['sources'].values():
        assert digest(read(ROOT / ('tools/rhino_oracle/observations/source_snapshots/' + sha + '.gz'))) == sha
    checks = receipt['checks']
    assert set(checks) == {'portable', 'native', 'kernel', 'python', 'app', 'format', 'capture'}
    assert all(item['exit_code'] == 0 for item in checks.values())
    assert 'test result: ok. 28 passed; 0 failed;' in read(ROOT / checks['kernel']['log']).decode()
    assert 'Ran 2 tests' in read(ROOT / checks['python']['log']).decode()
    assert checks['capture']['command'].startswith('tools/rhino_oracle/run_headless.sh exec ')
    assert 'Recorded surface_fit' in read(ROOT / checks['capture']['log']).decode()
    return dict(full_rhino_fit_parity=False, adaptive_basis_qualified=False,
                captured_bases_supplied_by_test=True, observations=36,
                backends={backend: summarize(query, native, read(ROOT / checks[backend]['log']).decode())
                          for backend in ['portable', 'native']},
                baseline_revision=receipt['baseline_revision'], receipts='surface-fit-basis-proof/receipts.json')


if __name__ == '__main__':
    print(json.dumps(report(), indent=2, allow_nan=False))
