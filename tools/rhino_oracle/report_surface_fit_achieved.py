"""Audit public SDK reports separately from complete bounds and sampled witnesses."""
import json
import math
from .report_surface_fit_basis import ROOT,read,digest

PROOF=ROOT/'docs/surface-fit-achieved-proof'
SOURCE_OVERRIDES=['crates/viboceros-geometry/src/'+p for p in ['lib.rs','nurbs_surface.rs',
    'nurbs_surface/closest_point.rs','nurbs_surface/deviation_samples.rs','nurbs_surface/deviation_samples/tests.rs']]+[
    'crates/viboceros-oracle/'+p for p in ['src/lib.rs','src/surface_deviation_samples.rs',
    'tests/surface_fit_achieved_native.rs','examples/surface_fit_metric_experiment.rs']]+[
    'tools/rhino_oracle/'+p for p in ['surface_fit_achieved_probe.py','generate_surface_fit_achieved.py',
    'public_surface_fit_signature.cpp','structural_sdk.py','rhino_worker.py','test_surface_fit_achieved.py',
    'report_surface_fit_achieved.py','test_surface_fit_achieved_record.py']]


def rows(log,prefix):return [json.loads(s[len(prefix)+1:])for s in log.splitlines()if s.startswith(prefix+' ')]

def check_bound(value):
    assert type(value)in(int,float)and 0.<=value<=1e-9
    return value


def summarize(query,native,log):
    observed=rows(log,'FIT_ACHIEVED');operations=query['operations'];captured=native['results']
    assert len(observed)==len({r['id']for r in observed})==len(operations)==len(captured)==15
    assert [r['id']for r in observed]==[o['id']for o in operations]==[r['id']for r in captured]
    maximum_authored=maximum_native=maximum_archive=0.;nonnull=nulls=source_unproved=0
    for op,capture,row in zip(operations,captured,observed):
        value=capture['value'];surface=value['surface'];limit=op['fit_tolerance']
        assert value['before']==value['after']and value['module']=='RhinoCore.dll'
        assert json.dumps(surface,sort_keys=True)==json.dumps(value['managed_surface'],sort_keys=True)
        assert value['achieved_finite']is True and row['reported_achieved']==value['achieved_tolerance']
        assert row['requested_tolerance']==limit and 0.<=row['reported_achieved']<=limit
        assert row['sampled_certified']is False
        maximum_authored=max(maximum_authored,check_bound(row['authored_bound']))
        if surface is None:
            assert row['native_bound']is row['archive_bound']is row['samples']is row['counts']is row['source_parameter_bound']is None
            assert row['reported_achieved']==0.;nulls+=1;continue
        nonnull+=1;assert row['counts']==surface['control_count']
        maximum_native=max(maximum_native,check_bound(row['native_bound']))
        maximum_archive=max(maximum_archive,check_bound(row['archive_bound']))
        bound=row['source_parameter_bound']
        if bound is None:source_unproved+=1
        else:assert 0.<=bound<=limit
        assert len(row['samples'])==3 and all(type(v)in(int,float)and math.isfinite(v)and v>=0. for v in row['samples'])
        # A retained seed is itself a witness; local refinement may only improve it.
        assert row['samples'][1]<=row['samples'][0]and row['samples'][2]<=row['samples'][0]
    assert(nonnull,nulls)==(14,1)
    assert rows(log,'FIT_ACHIEVED_TOTAL')==[dict(observations=15,captured_bases=14,native_null=1,adaptive_fit_parity=False)]
    assert 'test result: ok. 3 passed; 0 failed;'in log
    by_id={r['id']:r['value']for r in captured}
    pairs=[('achieved-strip-sweep-0.03','achieved-strip-sweep-0.05'),
        ('achieved-fit-strip-rational_bezier-0-3-0.01','achieved-fit-strip-rational_bezier-0-3-0.001')]
    for a,b in pairs:
        a=by_id[a];b=by_id[b]
        assert a['before']==b['before']and json.dumps(a['surface'],sort_keys=True)==json.dumps(b['surface'],sort_keys=True)
        assert a['achieved_tolerance']!=b['achieved_tolerance']
    return dict(captured_bases=14,native_null=1,cpp_managed_identical=15,identical_output_report_pairs=2,
        source_parameter_unproved=source_unproved,maximum_authored_input_bound=maximum_authored,
        maximum_native_basis_bound=maximum_native,maximum_archive_bound=maximum_archive)


def report():
    receipt=json.loads((PROOF/'receipts.json').read_bytes())
    assert set(receipt['source_overrides'])==set(SOURCE_OVERRIDES)
    for name,e in receipt['source_overrides'].items():assert digest(read(ROOT/e['snapshot']))==e['sha256'],name
    checks=receipt['checks'];assert set(checks)=={'portable','native','kernel','closest','python','app','format','capture','experiment'}
    assert all(e['exit_code']==0 and e['log']in receipt['files']for e in checks.values())
    required={e['log']for e in checks.values()}
    required.update(e['log']for e in receipt['rejected_runs'])
    required.update(e['capsule']for e in receipt['rejected_runs'])
    fixture='tools/rhino_oracle/fixtures/surface_fit_achieved.json';observation='tools/rhino_oracle/observations/surface_fit_achieved.json.gz'
    required.update([fixture,observation]);assert required.issubset(receipt['files'])
    for name,sha in receipt['files'].items():assert digest((ROOT/name).read_bytes())==sha,name
    query_bytes=(ROOT/fixture).read_bytes();q=json.loads(query_bytes);n=json.loads(read(ROOT/observation))
    def producer(capture):
        assert capture['provenance']['request_sha256']==digest(query_bytes)
        for sha in capture['provenance']['sources'].values():
            assert digest(read(ROOT/('tools/rhino_oracle/observations/source_snapshots/'+sha+'.gz')))==sha
    producer(n)
    assert checks['capture']['command'].startswith('tools/rhino_oracle/run_headless.sh exec ')
    assert 'Recorded surface_fit_achieved'in read(ROOT/checks['capture']['log']).decode()
    assert len(receipt['rejected_runs'])==1
    for rejected in receipt['rejected_runs']:
        assert rejected['exit_code']==1 and rejected['command'].startswith('tools/rhino_oracle/run_headless.sh exec ')
        failed=json.loads(read(ROOT/rejected['capsule']));producer(failed)
        assert 'Attempted relative import in non-package'in failed['error']
    assert 'test result: ok. 5 passed; 0 failed;'in read(ROOT/checks['kernel']['log']).decode()
    assert 'test result: ok. 23 passed; 0 failed;'in read(ROOT/checks['closest']['log']).decode()
    assert 'Ran 3 tests'in read(ROOT/checks['python']['log']).decode()
    assert 'RhinoFitSurface@@YAPEAVON_Surface@@AEBV1@HHNPEAN@Z'in read(ROOT/receipt['abi_ir']).decode()
    experiment=rows(read(ROOT/checks['experiment']['log']).decode(),'FIT_METRICS')
    assert len(experiment)==54 and len({r['id']for r in experiment})==9
    for r in experiment:
        assert r['source_to_candidate_witness']<=r['sampled_parameter']
        assert r['candidate_to_source_witness']<=r['sampled_parameter']
    return dict(full_rhino_fit_parity=False,fit_srf_registered=False,existing_fitter_admission_changed=False,
        sampled_witnesses_are_complete_bounds=False,observations=15,
        backends={k:summarize(q,n,read(ROOT/checks[k]['log']).decode())for k in ['portable','native']},
        offline_metric_rows=54,baseline_revision=receipt['baseline_revision'],receipts='surface-fit-achieved-proof/receipts.json')


if __name__=='__main__':print(json.dumps(report(),indent=2,allow_nan=False))
