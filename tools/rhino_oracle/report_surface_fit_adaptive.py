"""Audit automatic input bounds separately from captured-basis reproduction."""
import collections
import json
from . import report_surface_fit_basis as basis

ROOT=basis.ROOT
PROOF=ROOT/'docs/surface-fit-adaptive-proof'
FAMILIES=['surface_fit','surface_fit_adaptive','surface_fit_strips']
SOURCE_OVERRIDES=basis.SOURCE_OVERRIDES+[
    'crates/viboceros-geometry/src/surface_fit/adaptive'+name for name in
    ['.rs','/refine.rs','/tests.rs']
]+[
    'crates/viboceros-geometry/src/surface_pullback/certificate/surface_pair'+name
    for name in ['.rs','/tests.rs']
]+['crates/viboceros-oracle/'+name for name in
   ['src/lib.rs','src/surface_fit_bounded.rs','tests/surface_fit_adaptive_native.rs','tests/surface_fit_bounded_protocol.rs']]+[
    'tools/rhino_oracle/'+name for name in ['generate_surface_fit_adaptive.py','generate_surface_fit_strips.py',
    'test_surface_fit_adaptive.py','report_surface_fit_adaptive.py','test_surface_fit_adaptive_record.py']]


def rows(log,prefix):
    return [json.loads(line[len(prefix)+1:])for line in log.splitlines()if line.startswith(prefix+' ')]


def data():
    result=[]
    for family in FAMILIES:
        path=ROOT/('tools/rhino_oracle/fixtures/'+family+'.json')
        query=json.loads(path.read_bytes())
        native=json.loads(basis.read(ROOT/('tools/rhino_oracle/observations/'+family+'.json.gz')))
        assert basis.digest(path.read_bytes())==native['provenance']['request_sha256']
        for sha in native['provenance']['sources'].values():
            assert basis.digest(basis.read(ROOT/('tools/rhino_oracle/observations/source_snapshots/'+sha+'.gz')))==sha
        assert [o['id']for o in query['operations']]==[r['id']for r in native['results']]
        result.append((query,native))
    return result


def summarize(captures,log):
    operations=[o for q,_ in captures for o in q['operations']]
    native=[r for _,n in captures for r in n['results']]
    automatic=rows(log,'AUTOMATIC_SURFACE_FIT')
    assert len(operations)==len(set(o['id']for o in operations))==112
    assert [r['id']for r in automatic]==[o['id']for o in operations]
    assert 'test result: ok. 4 passed; 0 failed;'in log
    counts=collections.Counter();maximum_archive=maximum_native=0.
    for op,capture,row in zip(operations,native,automatic):
        assert capture['value']['before']==capture['value']['after']
        assert row['maximum_controls']==[16,16] and row['limit']==op['fit_tolerance']
        fitted=row['source_bound']is not None
        native_exists=capture['value']['surface']is not None
        if fitted:
            assert 0.<=row['source_bound']<=row['limit']
            assert all(op['degree_'+a]+1<=n<=16 for a,n in zip(['u','v'],row['controls']))
            assert 0.<=row['archive_bound']<=1e-9
            maximum_archive=max(maximum_archive,row['archive_bound'])
            if row['native_bound']is not None:
                assert native_exists and 0.<=row['native_bound']<=1e-9
                maximum_native=max(maximum_native,row['native_bound'])
            status=('native_match'if row['native_bound']is not None else'native_correspondence_unproved')if native_exists else'native_null_with_fit'
        else:
            assert row['controls']is row['archive_bound']is row['native_bound']is None
            status='native_surface_without_fit'if native_exists else'both_declined'
        assert row['status']==status
        counts[status]+=1
    assert rows(log,'AUTOMATIC_SURFACE_FIT_TOTAL')==[dict(cases=112,counts=dict(counts),full_rhino_fit_parity=False)]
    expected_basis=[r['id']for _,n in captures[1:]for r in n['results']if r['value']['surface']is not None]
    supplied=rows(log,'ADAPTIVE_SURFACE_BASIS')
    assert len(expected_basis)==67 and [r['id']for r in supplied]==expected_basis
    for row in supplied:
        assert 0.<=row['native_bound']<=1e-9 and 0.<=row['archive_bound']<=1e-9
    represented=rows(log,'FIT_REPRESENTATION')
    native_by_id={r['id']:r['value']for r in native}
    expected_pairs=[o['id']for o in captures[1][0]['operations']if '-0-'in o['id']and'small-plane'not in o['id']]
    assert len(expected_pairs)==16 and [r['id']for r in represented]==expected_pairs
    for row in represented:
        assert row['refined_id']==row['id'].replace('-0-','-1-',1)
        assert 0.<=row['source_bound']<=1e-9
        assert row['native_inventory_changed']==(row['original_controls']!=row['refined_controls'])
        for key,ident in [('original_controls',row['id']),('refined_controls',row['refined_id'])]:
            surface=native_by_id[ident]['surface']
            assert row[key]==(None if surface is None else surface['control_count'])
    old_basis=basis.summarize(*captures[0],log)
    return dict(automatic_counts=dict(counts),certified_outputs=sum(counts[k]for k in
                ['native_match','native_correspondence_unproved','native_null_with_fit']),
                maximum_automatic_native_bound=maximum_native,maximum_automatic_archive_bound=maximum_archive,
                captured_bases=67+old_basis['captured_bases'],
                maximum_captured_basis_bound=max([old_basis['maximum_native_bound']]+[r['native_bound']for r in supplied]),
                equivalent_input_pairs=16,native_inventory_changes=sum(r['native_inventory_changed']for r in represented))


def report():
    receipt=json.loads((PROOF/'receipts.json').read_bytes())
    assert set(receipt['source_overrides'])==set(SOURCE_OVERRIDES)
    for name,entry in receipt['source_overrides'].items():
        assert basis.digest(basis.read(ROOT/entry['snapshot']))==entry['sha256'],name
    for name,sha in receipt['files'].items():assert basis.digest((ROOT/name).read_bytes())==sha,name
    for family in FAMILIES:
        for directory,suffix in [('fixtures','.json'),('observations','.json.gz')]:
            assert 'tools/rhino_oracle/'+directory+'/'+family+suffix in receipt['files']
    checks=receipt['checks']
    assert set(checks)=={'portable','native','kernel','certificates','python','app','format','capture_adaptive','capture_strips',
                        'protocol_portable','protocol_native','averages_portable','averages_native'}
    assert all(c['exit_code']==0 and c['log']in receipt['files']for c in checks.values())
    for key in ['capture_adaptive','capture_strips']:
        assert checks[key]['command'].startswith('tools/rhino_oracle/run_headless.sh exec ')
        assert 'Recorded surface_fit_'in basis.read(ROOT/checks[key]['log']).decode()
    assert 'test result: ok. 36 passed; 0 failed;'in basis.read(ROOT/checks['kernel']['log']).decode()
    assert 'test result: ok. 94 passed; 0 failed;'in basis.read(ROOT/checks['certificates']['log']).decode()
    for key in ['protocol_portable','protocol_native']:
        assert 'test result: ok. 2 passed; 0 failed;'in basis.read(ROOT/checks[key]['log']).decode()
    for key in ['averages_portable','averages_native']:
        assert 'test result: ok. 3 passed; 0 failed;'in basis.read(ROOT/checks[key]['log']).decode()
    for failed in receipt['rejected_runs']:assert failed['exit_code']!=0 and failed['log']in receipt['files']
    captures=data()
    native_count=sum(r['value']['surface']is not None for _,n in captures for r in n['results'])
    assert native_count==90
    backends={}
    for key in ['portable','native']:
        result=summarize(captures,basis.read(ROOT/checks[key]['log']).decode())
        averages=rows(basis.read(ROOT/checks['averages_'+key]['log']).decode(),'SURFACE_PAIR_CERTIFICATE')
        assert len(averages)==len(set(r['id']for r in averages))==119
        assert all(r['archives']==r['surfaces']==2 and 0.<=r['maximum_surface_bound']<=1e-9 for r in averages)
        result.update(averaging_regression_pairs=119,maximum_averaging_surface_bound=max(r['maximum_surface_bound']for r in averages))
        backends[key]=result
    return dict(full_rhino_fit_parity=False,automatic_policy='complete normalized UV source bound',observations=112,
                native_surfaces=90,native_null=22,automatic_test_maximum_controls=[16,16],
                backends=backends,
                baseline_revision=receipt['baseline_revision'],receipts='surface-fit-adaptive-proof/receipts.json')


if __name__=='__main__':print(json.dumps(report(),indent=2,allow_nan=False))
