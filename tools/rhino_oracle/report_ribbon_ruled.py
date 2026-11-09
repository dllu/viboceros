"""Audit complete ribbon images without conflating topology or command parity."""
import json
import math
from .report_surface_fit_basis import ROOT,read,digest
from .report_curve_loose_offset import settings

PROOF=ROOT/'docs/ribbon-ruled-proof'

def rows(log):return [json.loads(s.split(' ',1)[1])for s in log.splitlines()if s.startswith('RIBBON_RULED ')]
def bound(x):
    assert type(x)in(int,float)and math.isfinite(x)and 0.<=x<=1e-9
    return x

def summarize(q,n,log):
    observed=rows(log);assert len(observed)==len(q['operations'])==len(n['results'])==32
    assert len({r['id']for r in observed})==32
    assert 'test result: ok. 4 passed; 0 failed;'in log
    support=archive=incidence=0.;matched=orientations=0;gaps=[];flips=[]
    for op,capture,row in zip(q['operations'],n['results'],observed):
        assert op['id']==capture['id']==row['id'];v=capture['value'];settings(op,v)
        assert v['success']is True and v['returned_idle']is True
        assert [o for o in v['before']if o['source']]==[o for o in v['after']if o['source']]
        target=next(o for o in v['created']if not o['source']);g=target['brep']['geometry']
        assert row['native_faces']==len(g['faces'])and row['full_ribbon_command']is False and row['error']is None
        support=max(support,bound(row['support_coverage_bound']));archive=max(archive,bound(row['archive_bound']))
        if row['incidence_bound']is None:
            assert row['id']=='ribbon-native-polycurve-both'and row['counts']==[4,12,9]and len(g['faces'])==3
            assert row['native_face_orientation_equal']is None;gaps.append(row['id'])
        else:
            assert row['counts']==[len(g['faces']),len(g['edges']),len(g['vertices'])]
            incidence=max(incidence,bound(row['incidence_bound']));matched+=1
            assert all(m is not None for m in row['matches'])
            assert {m['native_index']for m in row['matches']}==set(range(len(g['faces'])))
            for m in row['matches']:bound(m['bound'])
        if row['native_face_orientation_equal']is True:orientations+=1
        elif row['native_face_orientation_equal']is False:flips.append(row['id'])
    assert matched==31 and orientations==30 and gaps==['ribbon-native-polycurve-both']and flips==['ribbon-loose-circle-both']
    return dict(complete_image_matches=32,incidence_matches=matched,face_reversal_matches=orientations,
        incidence_gap_ids=gaps,face_reversal_difference_ids=flips,maximum_image_bound=support,
        maximum_incidence_geometry_bound=incidence,maximum_archive_support_bound=archive)

def report():
    r=json.loads((PROOF/'receipts.json').read_bytes())
    assert len(r['source_overrides'])>=18
    for name,e in r['source_overrides'].items():assert digest(read(ROOT/e['snapshot']))==e['sha256'],name
    for name,sha in r['files'].items():assert digest((ROOT/name).read_bytes())==sha,name
    checks=r['checks'];assert set(checks)=={'portable','native','kernel','grid','partition','section_basis',
        'wall_portable','wall_native','app','app_native','format','capture'}
    assert all(e['exit_code']==0 and e['log']in r['files']for e in checks.values())
    assert checks['capture']['command'].startswith('tools/rhino_oracle/run_headless.sh exec ')
    qpath=ROOT/'tools/rhino_oracle/fixtures/ribbon_ruled.json';q=json.loads(qpath.read_bytes())
    n=json.loads(read(ROOT/'tools/rhino_oracle/observations/ribbon_ruled.json.gz'))
    assert n['provenance']['request_sha256']==digest(qpath.read_bytes())
    for sha in n['provenance']['sources'].values():
        assert digest(read(ROOT/('tools/rhino_oracle/observations/source_snapshots/'+sha+'.gz')))==sha
    logs={k:read(ROOT/e['log']).decode()for k,e in checks.items()}
    assert 'Recorded ribbon_ruled'in logs['capture']
    for key,count in [('kernel',5),('grid',4),('partition',12),('section_basis',2),('wall_portable',3),('wall_native',3)]:
        assert 'test result: ok. '+str(count)+' passed; 0 failed;'in logs[key]
    assert 'Finished `dev` profile'in logs['app']and'Finished `dev` profile'in logs['app_native']
    assert len(r['unqualified_builds'])==2
    for e in r['unqualified_builds']:assert e['exit_code']==101 and e['log']in r['files']
    return dict(baseline_revision=r['baseline_revision'],full_ribbon_command=False,ribbon_registered=False,
        non_loose_corner_policy_implemented=False,between_station_distance_certified=False,
        loop_winding_compared=False,output_domains_compared=False,new_headless_rhino_launches=1,
        backends={k:summarize(q,n,logs[k])for k in ['portable','native']},
        closed_boundary_vertex_regression=True,existing_loose_wall_regressions=True,
        receipts='ribbon-ruled-proof/receipts.json')

if __name__=='__main__':print(json.dumps(report(),indent=2,allow_nan=False))
