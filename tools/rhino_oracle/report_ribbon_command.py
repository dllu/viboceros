"""Audit registered Ribbon output policy without hiding ordinary CPlane gaps."""
import json
import math
from .report_surface_fit_basis import ROOT,read,digest
from .report_curve_loose_offset import settings

PROOF=ROOT/'docs/ribbon-command-proof'
FAMILIES=dict(ribbon_options=13,ribbon_workflows=12)
GAPS={'tilted-cplane','spatial-curve-plane','spatial-cplane'}


def bound(x):
    assert type(x) in (int,float) and math.isfinite(x) and 0.<=x<=1e-9
    return x


def summarize(corpus,log):
    marker='RIBBON_COMMAND '
    rows=[json.loads(s.split(marker,1)[1])for s in log.splitlines()if marker in s]
    assert len(rows)==len({(r['family'],r['id'])for r in rows})==25
    assert 'test result: ok. 1 passed; 0 failed;'in log
    maximum=incidence=0.;matched=0;gaps=[]
    for family,(q,n) in corpus.items():
        observed=[r for r in rows if r['family']==family]
        assert len(q['operations'])==len(n['results'])==len(observed)==FAMILIES[family]
        for op,capture,row in zip(q['operations'],n['results'],observed):
            assert op['id']==capture['id']==row['id']
            v=capture['value'];settings(op,v)
            assert v['success']is True and v['returned_idle']is True and row['native_success']is True
            assert [o for o in v['before']if o['source']]==[o for o in v['after']if o['source']]
            outputs=[o for o in v['created']if not o['source']]
            assert len(outputs)==1 and outputs[0]['valid']is True
            gap=family=='ribbon_workflows'and op['id']in GAPS
            assert row['local_supported']is(not gap)
            if gap:
                assert row['reason']=='ordinary_cplane_fitter'
                gaps.append(dict(family=family,id=op['id']))
            else:
                assert row['attributes_selection_history']is True and row['full_command_parity']is False
                maximum=max(maximum,bound(row['image_bound']))
                incidence=max(incidence,bound(row['incidence_bound']));matched+=1
    assert matched==22 and len(gaps)==3
    return dict(complete_image_and_incidence_matches=matched,attributes_selection_history_matches=matched,
        maximum_image_bound=maximum,maximum_incidence_bound=incidence,
        native_success_local_unsupported=gaps)


def report():
    r=json.loads((PROOF/'receipts.json').read_bytes())
    for name,e in r['source_overrides'].items():
        assert digest(read(ROOT/e['snapshot']))==e['sha256'],name
    for name,sha in r['files'].items():assert digest((ROOT/name).read_bytes())==sha,name
    checks=r['checks']
    assert set(checks)=={'portable','native','kernel','loose','smooth','ui','ui_native','app','app_native','format','probe','capture'}
    assert all(e['exit_code']==0 and e['log']in r['files']for e in checks.values())
    logs={k:read(ROOT/e['log']).decode()for k,e in checks.items()}
    for key,count in [('kernel',4),('ui',3),('ui_native',3),('loose',4),('smooth',3)]:
        assert 'test result: ok. '+str(count)+' passed; 0 failed;'in logs[key]
    assert all('Finished `dev` profile'in logs[k]for k in ['app','app_native'])
    assert 'Ran 3 tests'in logs['probe']
    assert checks['capture']['command'].startswith('tools/rhino_oracle/run_headless.sh exec ')
    assert 'Recorded ribbon_workflows'in logs['capture']
    corpus={}
    for family in FAMILIES:
        qp=ROOT/('tools/rhino_oracle/fixtures/'+family+'.json')
        q=json.loads(qp.read_bytes());n=json.loads(read(ROOT/('tools/rhino_oracle/observations/'+family+'.json.gz')))
        assert n['provenance']['request_sha256']==digest(qp.read_bytes())
        for sha in n['provenance']['sources'].values():
            assert digest(read(ROOT/('tools/rhino_oracle/observations/source_snapshots/'+sha+'.gz')))==sha
        corpus[family]=q,n
    assert all(e['qualified']is False and e['log']in r['files']for e in r['unqualified_runs'])
    return dict(baseline_revision=r['baseline_revision'],ribbon_registered=True,full_command_parity=False,
        new_headless_rhino_launches=1,new_native_workflows=12,
        between_station_distance_certified=False,loop_winding_compared=False,output_charts_compared=False,
        native_getter_transcripts_replayed=False,cursor_preview=False,through_point=False,
        custom_tolerance_native_qualified=False,
        trim_self_intersections=False,native_performance_parity=False,
        backends={k:summarize(corpus,logs[k])for k in ['portable','native']},
        receipts='ribbon-command-proof/receipts.json')


if __name__=='__main__':print(json.dumps(report(),indent=2,allow_nan=False))
