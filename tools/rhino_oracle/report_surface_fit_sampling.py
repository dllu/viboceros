"""Audit sampling equivalence, timing scope, and controlled native observations."""
import json
import math
from .generate_surface_fit_covariance import request
from .report_surface_fit_basis import ROOT, read, digest

PROOF = ROOT/'docs/surface-fit-sampling-proof'
FAMILY = 'surface_fit_covariance'
BASELINE_SOURCES = ['crates/viboceros-geometry/src/'+p for p in [
    'surface_fit.rs','surface_fit/adaptive.rs','surface_fit/adaptive/refine.rs',
    'nurbs_surface/evaluate/query.rs','surface_fit/tests.rs']]+[
    'crates/viboceros-oracle/tests/surface_fit_covariance_native.rs',
    'crates/viboceros-oracle/examples/surface_fit_sampling.rs']
SOURCE_OVERRIDES = BASELINE_SOURCES+['tools/rhino_oracle/'+p for p in [
    'generate_surface_fit_covariance.py','report_surface_fit_sampling.py',
    'test_surface_fit_sampling_record.py']]


def rows(log, prefix):
    return [json.loads(s[len(prefix)+1:]) for s in log.splitlines() if s.startswith(prefix+' ')]


def bound(value, limit):
    assert type(value) in (int, float) and math.isfinite(value) and 0. <= value <= limit
    return value


def summarize(query, native, log):
    operations = query['operations']; captures = native['results']
    assert len(operations) == len(captures) == 35
    assert [o['id'] for o in operations] == [r['id'] for r in captures]
    surface_ops = [o for o in operations if o['op'] == 'surface_fit_achieved']
    surface_captures = [r for o,r in zip(operations,captures) if o['op'] == 'surface_fit_achieved']
    observed = rows(log, 'FIT_COVARIANCE'); covariance = rows(log, 'FIT_COVARIANCE_NATIVE')
    assert len(observed) == len(covariance) == len(surface_ops) == 29
    assert 'test result: ok. 2 passed; 0 failed;' in log
    assert [o['id'] for o in surface_ops] == [r['id'] for r in observed] == [r['id'] for r in covariance]
    maximum_native = maximum_archive = maximum_covariance = maximum_authored = 0.
    for op, capture, row, cov in zip(surface_ops, surface_captures, observed, covariance):
        v = capture['value']; limit = op['fit_tolerance']
        assert v['before'] == v['after'] and v['surface'] == v['managed_surface']
        assert v['module'] == 'RhinoCore.dll' and v['achieved_finite'] is True
        bound(v['achieved_tolerance'], limit)
        assert row['limit'] == limit and row['full_native_policy'] is False
        bound(row['local_bound'], limit); assert row['local_bound'] == row['rechecked_bound']
        assert row['local_certificate'].startswith(('NormalizedUv','SeparableUv('))
        maximum_native = max(maximum_native, bound(row['native_bound'],1e-9))
        maximum_authored = max(maximum_authored, bound(row['authored_bound'],1e-9))
        maximum_archive = max(maximum_archive, bound(row['archive_bound'],1e-9))
        maximum_covariance = max(maximum_covariance, bound(cov['bound'],1e-9))
        assert row['captured_basis']['counts'] == v['surface']['control_count']
        assert row['captured_basis']['degree'] == row['local_surface']['degree'] == [3,op['degree_v']]
        assert all(p[1] == 1. for p in row['local_surface']['controls'])
        assert row['local_surface']['counts'][1] == op['degree_v']+1
    curves = rows(log,'FIT_COVARIANCE_CURVE')
    curve_captures = [(o,r) for o,r in zip(operations,captures) if o['op'] == 'curve_fit_geometry']
    assert len(curves) == len(curve_captures) == 6
    assert len({o['id'] for o in operations}) == 35
    for (op, capture), row in zip(curve_captures,curves):
        v = capture['value']; a,b = v['domain']
        knots = [(t-a)/(b-a) for t in v['knots']]
        assert row == dict(id=op['id'],controls=len(v['control_points']),knots=knots)
        assert v['degree'] == op['degree'] and any(abs(t*4.-round(t*4.))>1e-6 for t in knots)
    return dict(surface_observations=29,curve_observations=6,certified_local_fits=29,
        captured_basis_matches=29,complete_native_covariance_matches=29,
        maximum_authored_bound=maximum_authored,maximum_native_basis_bound=maximum_native,
        maximum_archive_bound=maximum_archive,maximum_native_covariance_bound=maximum_covariance)


def timings(before_log, after_log):
    before = rows(before_log,'FIT_SAMPLING'); after = rows(after_log,'FIT_SAMPLING')
    assert [r['controls_per_axis'] for r in before] == [r['controls_per_axis'] for r in after] == [16,32,64]
    result = []
    for a,b in zip(before,after):
        for r in [a,b]:
            assert r['iterations'] == len(r['times_seconds']) == 5
            assert r['source_degree'] == [8,8] and r['source_controls'] == [12,12]
            assert r['output_degree'] == [3,3] and r['source_accuracy_certified'] is False
            assert r['times_seconds'] == sorted(r['times_seconds'])
            assert all(math.isfinite(t) and t>0. for t in r['times_seconds'])
            assert r['median_seconds'] == r['times_seconds'][2]
        assert a['fingerprint'] == b['fingerprint']
        result.append(dict(controls_per_axis=a['controls_per_axis'],fingerprint=a['fingerprint'],
            before_median_seconds=a['median_seconds'],after_median_seconds=b['median_seconds'],
            speedup=a['median_seconds']/b['median_seconds']))
    return result


def report():
    receipt = json.loads((PROOF/'receipts.json').read_bytes())
    assert set(receipt['source_overrides']) == set(SOURCE_OVERRIDES)
    assert set(receipt['baseline_sources']) == set(BASELINE_SOURCES)
    for name,sha in receipt['files'].items(): assert digest((ROOT/name).read_bytes()) == sha,name
    for group in ['source_overrides','baseline_sources']:
        for name,e in receipt[group].items(): assert digest(read(ROOT/e['snapshot'])) == e['sha256'],name
    checks = receipt['checks']
    required = {e['log'] for e in checks.values()}|{e['log'] for e in receipt['unqualified_runs']}
    required.update(e['snapshot'] for group in ['source_overrides','baseline_sources'] for e in receipt[group].values())
    required.update(['tools/rhino_oracle/'+kind+'/'+FAMILY+suffix
                     for kind,suffix in [('fixtures','.json'),('observations','.json.gz')]])
    assert required.issubset(receipt['files'])
    assert set(checks) == {'baseline','portable','native','kernel','evaluate','sampling_before',
        'sampling_after','app','app_native','format','capture','probe_tests'}
    assert all(e['exit_code'] == 0 and e['log'] in receipt['files'] for e in checks.values())
    assert len(receipt['unqualified_runs']) == 2
    for run in receipt['unqualified_runs']:
        assert run['exit_code'] == 101 and run['qualified'] is False
        assert 'E0282' in read(ROOT/run['log']).decode()
    fixture = ROOT/('tools/rhino_oracle/fixtures/'+FAMILY+'.json')
    q = json.loads(fixture.read_bytes()); assert q == request()
    n = json.loads(read(ROOT/('tools/rhino_oracle/observations/'+FAMILY+'.json.gz')))
    assert n['provenance']['request_sha256'] == digest(fixture.read_bytes())
    for sha in n['provenance']['sources'].values():
        assert digest(read(ROOT/('tools/rhino_oracle/observations/source_snapshots/'+sha+'.gz'))) == sha
    assert checks['capture']['command'].startswith('tools/rhino_oracle/run_headless.sh exec ')
    assert 'Recorded surface_fit_covariance' in read(ROOT/checks['capture']['log']).decode()
    logs = {k:read(ROOT/e['log']).decode() for k,e in checks.items()}
    backends = {k:summarize(q,n,logs[k]) for k in ['baseline','portable','native']}
    for prefix in ['FIT_COVARIANCE','FIT_COVARIANCE_NATIVE','FIT_COVARIANCE_CURVE']:
        assert rows(logs['baseline'],prefix) == rows(logs['portable'],prefix),prefix
    assert 'test result: ok. 56 passed; 0 failed;' in logs['kernel']
    assert 'test result: ok. 60 passed; 0 failed; 2 ignored;' in logs['evaluate']
    assert 'Ran 5 tests' in logs['probe_tests']
    assert 'Finished `dev` profile' in logs['app'] and 'Finished `dev` profile' in logs['app_native']
    return dict(baseline_revision=receipt['baseline_revision'],full_native_fit_policy=False,
        fit_srf_registered=False,private_xvfb_capture=True,visible_desktop_windows_opened=0,
        backends=backends,portable_rows_unchanged=29,
        fitting_tests=56,evaluator_tests=dict(passed=60,ignored_manual_benchmarks=2),
        benchmark_scope='Unoptimized dev build; full explicit interpolation, one degree-8 rational tensor; five timed runs per size.',
        benchmark=timings(logs['sampling_before'],logs['sampling_after']),
        receipts='surface-fit-sampling-proof/receipts.json')


if __name__ == '__main__':
    print(json.dumps(report(),indent=2,allow_nan=False))
