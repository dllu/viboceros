"""Audit complete loose-offset curves separately from unfinished Ribbon assembly."""
import json
import math
from .report_surface_fit_basis import ROOT,read,digest

PROOF=ROOT/'docs/curve-loose-offset-proof'
FAMILIES=['curve_loose_offset','curve_loose_offset_types']
SOURCES=['crates/viboceros-geometry/src/nurbs/'+p for p in ['loose_offset.rs','loose_offset/tests.rs']]+[
    'crates/viboceros-oracle/'+p for p in ['src/lib.rs','src/curve_loose_offset.rs','tests/curve_loose_offset_native.rs']]+[
    'tools/rhino_oracle/'+p for p in ['ribbon_probe.py','generate_ribbon.py','generate_ribbon_options.py',
    'generate_curve_loose_offset.py','generate_curve_loose_offset_types.py','rhino_worker.py','structural_sdk.py',
    'test_ribbon_probe.py','report_curve_loose_offset.py','test_curve_loose_offset_record.py','run_headless.sh']]


def rows(log,prefix):
    return [json.loads(s[len(prefix)+1:])for s in log.splitlines()if s.startswith(prefix+' ')]


def bound(x):
    assert type(x)in(int,float)and math.isfinite(x)and 0.<=x<=1e-9
    return x


def captures(family):
    q=json.loads((ROOT/('tools/rhino_oracle/fixtures/'+family+'.json')).read_bytes())
    n=json.loads(read(ROOT/('tools/rhino_oracle/observations/'+family+'.json.gz')))
    return q,n

def settings(op,value):
    assert 'Unknown command'not in value['history']
    prompts=[s for s in value['history'].splitlines()if s.startswith('Side to offset (')]
    assert prompts
    last=prompts[-1]
    assert ('Loose='+('Yes'if op['loose']else'No'))in last
    assert ('InCPlane='+('Yes'if op['in_cplane']else'No'))in last
    assert ('OutputLayer='+op['output_layer'])in last
    assert ('BothSides'in last)is(not op['both_sides'])
    if not op['loose']:assert ('Corner='+op['corner'])in last


def summarize(corpus,log):
    observed=rows(log,'CURVE_LOOSE_OFFSET')
    assert len(observed)==29 and len({(r['id'],r['native_index'])for r in observed})==29
    assert 'test result: ok. 3 passed; 0 failed;'in log
    maximum=0.;consumed=0
    for q,n in corpus:
        assert len(q['operations'])==len(n['results'])
        for op,capture in zip(q['operations'],n['results']):
            assert op['id']==capture['id']
            v=capture['value'];assert v['success']is True and v['returned_idle']is True
            settings(op,v)
            original=[o for o in v['before']if o['source']]
            assert original==[o for o in v['after']if o['source']]
            targets=[o for o in v['created']if not o['source']]
            actual=[r for r in observed if r['id']==op['id']]
            assert len(actual)==len(targets)==(2 if op['both_sides']else 1)
            assert {r['native_index']for r in actual}==set(range(len(targets)))
            for row in actual:
                target=targets[row['native_index']]['curve']
                assert abs(row['distance'])==op['distance']
                assert row['degree']==target['degree'] and row['controls']==row['native_controls']==len(target['control_points'])
                assert all(p['weight']==1. for p in target['control_points'])
                maximum=max(maximum,bound(row['native_bound']));consumed+=1
    assert consumed==29 and sum(len(q['operations'])for q,_ in corpus)==26
    ribbon=rows(log,'RIBBON_LOOSE_BOUNDARY')
    assert len(ribbon)==1 and ribbon[0]['id']=='smooth-loose'and ribbon[0]['complete_ribbon_command']is False
    return dict(workflows=26,complete_offset_matches=29,maximum_native_curve_bound=maximum,
        native_ribbon_boundary_bound=bound(ribbon[0]['native_bound']))


def provenance(data,fixture=None):
    p=data['provenance']
    if fixture is not None:assert p['request_sha256']==digest(fixture.read_bytes())
    for sha in p['sources'].values():
        assert digest(read(ROOT/('tools/rhino_oracle/observations/source_snapshots/'+sha+'.gz')))==sha


def report():
    r=json.loads((PROOF/'receipts.json').read_bytes())
    assert set(r['source_overrides'])==set(SOURCES)
    required=set()
    for name,e in r['source_overrides'].items():
        assert digest(read(ROOT/e['snapshot']))==e['sha256'],name
        required.add(e['snapshot'])
    checks=r['checks'];assert set(checks)=={'portable','native','kernel','inset_portable','inset_native','python','app','app_native','format',
        'ribbon_capture','ribbon_options_capture','offset_capture','types_capture'}
    assert all(e['exit_code']==0 for e in checks.values())
    assert checks['ribbon_capture']['qualified']is False
    assert all(e['qualified']is True for key,e in checks.items()if key!='ribbon_capture')
    required.update(e['log']for e in checks.values())
    for name,sha in r['files'].items():assert digest((ROOT/name).read_bytes())==sha,name
    assert required.issubset(r['files'])
    for family in ['ribbon','ribbon_options',*FAMILIES]:
        q,n=captures(family)
        provenance(n,ROOT/('tools/rhino_oracle/fixtures/'+family+'.json'))
        assert [o['id']for o in q['operations']]==[v['id']for v in n['results']]
    q,ribbon=captures('ribbon_options');assert len(q['operations'])==13
    for op,v in zip(q['operations'],ribbon['results']):
        assert v['value']['success']is True;settings(op,v['value'])
    _,exploratory=captures('ribbon')
    assert len(exploratory['results'])==13 and all('Unknown command'in v['value']['history']for v in exploratory['results'])
    assert len(r['failed_captures'])==3
    for e in r['failed_captures']:
        failed=json.loads(read(ROOT/e['capsule']));provenance(failed)
        assert e['capsule']in r['files']and e['log']in r['files']and e['exit_code']==1
        assert e['command'].startswith('tools/rhino_oracle/run_headless.sh exec ')
    assert len(r['unqualified_builds'])==4
    for e in r['unqualified_builds']:
        assert e['qualified']is False and e['log']in r['files']and e['exit_code']==101
    logs={key:read(ROOT/e['log']).decode()for key,e in checks.items()}
    for key in ['ribbon_capture','ribbon_options_capture','offset_capture','types_capture']:
        assert checks[key]['command'].startswith('tools/rhino_oracle/run_headless.sh exec ')
        assert 'Recorded 'in logs[key]
    assert 'test result: ok. 4 passed; 0 failed;'in logs['kernel']
    assert 'test result: ok. 1 passed; 0 failed;'in logs['inset_portable']
    assert 'test result: ok. 2 passed; 0 failed;'in logs['inset_native']
    assert 'Ran 3 tests'in logs['python']
    assert 'Finished `dev` profile'in logs['app']and'Finished `dev` profile'in logs['app_native']
    corpus=[captures(f)for f in FAMILIES]
    return dict(baseline_revision=r['baseline_revision'],ribbon_registered=False,
        offset_command_loose_option_registered=False,between_station_distance_certified=False,
        native_output_order_compared=False,native_output_domains_compared=False,
        ribbon_policy_captures=13,unqualified_toggle_captures=13,failed_captures_retained=3,
        headless_launches=7,visible_desktop_windows_opened=0,
        backends={k:summarize(corpus,logs[k])for k in ['portable','native']},
        retained_weight_inset_regression=True,python_definition_roundtrip=True,
        receipts='curve-loose-offset-proof/receipts.json')


if __name__=='__main__':print(json.dumps(report(),indent=2,allow_nan=False))
