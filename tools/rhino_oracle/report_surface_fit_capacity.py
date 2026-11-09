"""Audit equal-degree budget observations without claiming adaptive fitting parity."""
import json
from .report_surface_fit_basis import ROOT,read,digest

PROOF=ROOT/'docs/surface-fit-capacity-proof'
NEW=['surface_fit_capacity','surface_fit_capacity_holdouts']
PRIOR=['surface_fit','surface_fit_adaptive','surface_fit_strips']
SOURCE_OVERRIDES=['crates/viboceros-geometry/src/'+name for name in
    ['lib.rs','surface_fit.rs','surface_fit/budget.rs','surface_fit/budget/tests.rs']]+[
    'crates/viboceros-oracle/'+name for name in
    ['src/lib.rs','src/surface_fit_budget.rs','tests/surface_fit_capacity_native.rs']]+[
    'tools/rhino_oracle/'+name for name in ['generate_surface_fit_capacity.py','generate_surface_fit_capacity_holdouts.py',
    'test_surface_fit_capacity.py','report_surface_fit_capacity.py','test_surface_fit_capacity_record.py']]


def rows(log,prefix):
    return [json.loads(line[len(prefix)+1:])for line in log.splitlines()if line.startswith(prefix+' ')]


def ceiling(definition,degree):
    if definition['degree']!=degree:return None
    result=[]
    for p,n,k in zip(degree,definition['control_count'],[definition['knots_u'],definition['knots_v']]):
        if any(t!=k[p]for t in k[:p+1])or any(t!=k[n]for t in k[n:]):return None
        result.append(p+sum(a<b for a,b in zip(k[p:n],k[p+1:n+1])))
    return result


def admits(count,initial,maximum):
    return count==initial or all(a<=n<=b for n,a,b in zip(count,initial,maximum))and any(n<b for n,b in zip(count,maximum))


def data(families):
    result=[]
    for f in families:
        fixture=ROOT/('tools/rhino_oracle/fixtures/'+f+'.json');q=json.loads(fixture.read_bytes())
        n=json.loads(read(ROOT/('tools/rhino_oracle/observations/'+f+'.json.gz')))
        assert digest(fixture.read_bytes())==n['provenance']['request_sha256']
        for sha in n['provenance']['sources'].values():
            assert digest(read(ROOT/('tools/rhino_oracle/observations/source_snapshots/'+sha+'.gz')))==sha
        assert [o['id']for o in q['operations']]==[r['id']for r in n['results']]
        result.extend(zip(q['operations'],n['results']))
    return result


def summarize(captures,prior,log):
    observed=rows(log,'FIT_CAPACITY');old=rows(log,'PRIOR_FIT_CAPACITY')
    assert len(observed)==len({r['id']for r in observed})==len(captures)==57
    assert [r['id']for r in observed]==[o['id']for o,_ in captures]
    matched=nulls=qualified=unknown=0;maximum=[0.,0.,0.,0.]
    for(op,n),r in zip(captures,observed):
        value=n['value'];source=value['before'];target=value['surface'];degree=[op['degree_u'],op['degree_v']]
        assert source==value['after']
        cap=ceiling(source,degree)
        assert r['maximum_controls']==cap and r['source_count']==source['control_count']
        for i,key in enumerate(['source_bound','authored_bound']):
            assert type(r[key])in(int,float)and 0.<=r[key]<=1e-9
            maximum[i]=max(maximum[i],r[key])
        if target is None:
            assert r['native_count']is r['native_bound']is r['archive_bound']is r['candidate_admitted']is None
            nulls+=1;continue
        matched+=1;count=target['control_count']
        assert target['degree']==degree
        assert r['native_count']==count
        if cap is None:
            assert r['candidate_admitted']is None;unknown+=1
        else:
            assert r['candidate_admitted']is True and admits(count,[p+1 for p in degree],cap);qualified+=1
        for i,key in enumerate(['native_bound','archive_bound'],2):
            assert type(r[key])in(int,float)and 0.<=r[key]<=1e-9
            maximum[i]=max(maximum[i],r[key])
    assert(matched,nulls,qualified,unknown)==(18,39,17,1)
    assert rows(log,'FIT_CAPACITY_TOTAL')==[dict(observations=57,captured_bases=18,native_null=39,adaptive_fit_parity=False)]
    expected=[(o,n)for o,n in prior if n['value']['surface']is not None]
    assert len(old)==len({r['id']for r in old})==len(expected)==90
    prior_qualified=prior_unknown=0
    for(op,n),r in zip(expected,old):
        assert op['id']==r['id'];v=n['value'];p=[op['degree_u'],op['degree_v']];cap=ceiling(v['before'],p)
        assert r['maximum_controls']==cap
        if cap is None:
            assert r['candidate_admitted']is None;prior_unknown+=1
        else:
            assert r['candidate_admitted']is True and admits(v['surface']['control_count'],[d+1 for d in p],cap)
            prior_qualified+=1
    assert(prior_qualified,prior_unknown)==(45,45)
    assert rows(log,'PRIOR_FIT_CAPACITY_TOTAL')==[dict(qualified=45,degree_change_unknown=45)]
    assert 'test result: ok. 3 passed; 0 failed;'in log
    return dict(observations=57,captured_bases=18,native_null=39,equal_degree_outputs_admitted=17,degree_change_output_unknown=1,
        prior_equal_degree_outputs_admitted=45,prior_degree_change_outputs_unknown=45,
        maximum_source_equivalence_bound=maximum[0],maximum_authored_input_bound=maximum[1],
        maximum_native_basis_bound=maximum[2],maximum_archive_bound=maximum[3])


def report():
    receipt=json.loads((PROOF/'receipts.json').read_bytes())
    assert set(receipt['source_overrides'])==set(SOURCE_OVERRIDES)
    for name,e in receipt['source_overrides'].items():assert digest(read(ROOT/e['snapshot']))==e['sha256'],name
    checks=receipt['checks'];assert set(checks)=={'portable','native','kernel','python','app','format','capture','capture_holdouts'}
    assert all(c['exit_code']==0 and c['log']in receipt['files']for c in checks.values())
    required={c['log']for c in checks.values()}
    for f in NEW+PRIOR:required.update('tools/rhino_oracle/'+directory+'/'+f+suffix for directory,suffix in [('fixtures','.json'),('observations','.json.gz')])
    assert required.issubset(receipt['files'])
    for name,sha in receipt['files'].items():assert digest((ROOT/name).read_bytes())==sha,name
    assert 'test result: ok. 50 passed; 0 failed;'in read(ROOT/checks['kernel']['log']).decode()
    assert 'Ran 3 tests'in read(ROOT/checks['python']['log']).decode()
    for key,family in [('capture',NEW[0]),('capture_holdouts',NEW[1])]:
        assert checks[key]['command'].startswith('tools/rhino_oracle/run_headless.sh exec ')
        assert 'Recorded '+family in read(ROOT/checks[key]['log']).decode()
    return dict(full_rhino_fit_parity=False,fit_srf_registered=False,existing_fitter_policy_changed=False,
        budget_is_accuracy_certificate=False,budget_is_null_predictor=False,
        supported_policy='clamped continuous equal-degree source bases',observations=57,
        backends={k:summarize(data(NEW),data(PRIOR),read(ROOT/checks[k]['log']).decode())for k in ['portable','native']},
        baseline_revision=receipt['baseline_revision'],receipts='surface-fit-capacity-proof/receipts.json')


if __name__=='__main__':print(json.dumps(report(),indent=2,allow_nan=False))
