"""Audit automatic image accuracy, UV regression and separate native agreement."""
import collections
import json
import math
from .report_surface_fit_basis import ROOT, read, digest

PROOF = ROOT/'docs/surface-image-auto-proof'
FAMILIES = ['surface_fit','surface_fit_adaptive','surface_fit_strips']


def rows(log, prefix):
    return [json.loads(line[len(prefix)+1:]) for line in log.splitlines() if line.startswith(prefix+' ')]


def bound(value, limit):
    assert type(value) in (float,int) and math.isfinite(value) and 0. <= value <= limit
    return value


def data():
    return [(json.loads((ROOT/('tools/rhino_oracle/fixtures/'+f+'.json')).read_bytes()),
        json.loads(read(ROOT/('tools/rhino_oracle/observations/'+f+'.json.gz')))) for f in FAMILIES]


def maps_valid(certificate):
    assert certificate['type'] in ['normalized_uv','planar_quad','separable_uv']
    if certificate['type'] != 'separable_uv': return
    maps = certificate['correspondence']; assert len(maps) == 2
    for pairs in maps:
        assert 2 <= len(pairs) <= 17 and pairs[0] == [0.,0.] and pairs[-1] == [1.,1.]
        assert all(len(p) == 2 and all(type(t) in (int,float) and math.isfinite(t) for t in p) for p in pairs)
        assert all(all(a[k] < b[k] for k in [0,1]) for a,b in zip(pairs,pairs[1:]))


def uv_summary(captures, log):
    observed = rows(log,'AUTOMATIC_SURFACE_FIT')
    assert len(observed) == len({r['id'] for r in observed}) == 112
    assert 'test result: ok. 1 passed; 0 failed;' in log
    counts = collections.Counter()
    for op, native, row in zip([o for q,_ in captures for o in q['operations']],
        [r for _,n in captures for r in n['results']], observed):
        assert op['id'] == native['id'] == row['id'] and row['limit'] == op['fit_tolerance']
        fitted = row['source_bound'] is not None; exists = native['value']['surface'] is not None
        if fitted:
            bound(row['source_bound'],row['limit']); bound(row['archive_bound'],1e-9)
        else: assert row['controls'] is row['archive_bound'] is row['native_bound'] is None
        if row['native_bound'] is not None: bound(row['native_bound'],1e-9); assert fitted and exists
        expected = ('native_match' if row['native_bound'] is not None else 'native_correspondence_unproved') if fitted and exists else (
            'native_null_with_fit' if fitted else 'native_surface_without_fit' if exists else 'both_declined')
        assert row['status'] == expected; counts[expected] += 1
    assert counts == dict(native_match=35,native_correspondence_unproved=40,native_null_with_fit=12,native_surface_without_fit=15,both_declined=10)
    assert rows(log,'AUTOMATIC_SURFACE_FIT_TOTAL') == [dict(cases=112,counts=dict(counts),full_rhino_fit_parity=False)]
    return dict(certified_outputs=87,counts=dict(counts))


def image_summary(captures, log, baseline_log):
    observed = rows(log,'SURFACE_IMAGE_FIT'); previous = rows(baseline_log,'SURFACE_IMAGE_FIT')
    assert len(observed) == len(previous) == len({r['id'] for r in observed}) == 112
    assert 'test result: ok. 3 passed; 0 failed;' in log
    counts = collections.Counter(); certificates = collections.Counter(); added=[]; reduced=[]; gained=[]; lost=[]
    maximum_native=maximum_archive=0.; planar=0
    for op,native,old,row in zip([o for q,_ in captures for o in q['operations']],
        [r for _,n in captures for r in n['results']],previous,observed):
        assert op['id'] == native['id'] == old['id'] == row['id']
        value=native['value']; assert value['before'] == value['after']
        exists = value['surface'] is not None; fitted = row['source_bound'] is not None
        flat = all(p['point'][2] == 0. for p in value['before']['control_points'])
        assert row['planar_fixture'] is flat and row['maximum_controls'] == [16,16] and row['limit'] == op['fit_tolerance']
        agreement = [bound(row[k],1e-9) for k in ['native_parameter_bound','native_planar_bound'] if row[k] is not None]
        if old['source_bound'] is not None:
            assert fitted and all(a<=b for a,b in zip(row['controls'],old['controls']))
        if fitted:
            bound(row['source_bound'],row['limit']); assert row['source_rechecked_bound'] == row['source_bound']
            maps_valid(row['certificate']); certificates[row['certificate']['type']] += 1
            maximum_archive=max(maximum_archive,bound(row['archive_bound'],1e-9))
            assert len(row['controls']) == 2 and all(op['degree_'+a]+1 <= n <= 16 for a,n in zip(['u','v'],row['controls']))
            if old['source_bound'] is None: added.append(row['id'])
            elif math.prod(row['controls']) < math.prod(old['controls']): reduced.append(row['id'])
        else:
            assert row['source_rechecked_bound'] is row['certificate'] is row['controls'] is row['archive_bound'] is None
        if agreement:
            assert fitted and exists; maximum_native=max(maximum_native,min(agreement))
        if not exists: assert not agreement
        expected = ('native_image_match' if agreement else 'native_image_unproved') if fitted and exists else (
            'native_null_with_fit' if fitted else 'native_surface_without_fit' if exists else 'both_declined')
        assert row['status'] == expected; counts[expected] += 1
        if old['status'] == 'native_image_match' and expected != 'native_image_match': lost.append(row['id'])
        if old['status'] != 'native_image_match' and expected == 'native_image_match': gained.append(row['id'])
        if flat:
            assert fitted and exists and row['native_planar_bound'] is not None
            assert row['controls'] == [op['degree_u']+1,op['degree_v']+1]; planar += 1
    assert planar == 30
    assert rows(log,'SURFACE_IMAGE_FIT_TOTAL') == [dict(cases=112,counts=dict(counts),planar_native_matches=30,full_rhino_fit_parity=False)]
    return dict(certified_outputs=sum(certificates.values()),counts=dict(counts),certificates=dict(certificates),
        planar_single_patch_native_matches=planar,newly_fitted_ids=added,reduced_control_ids=reduced,
        native_match_gained_ids=gained,newly_unproved_native_ids=lost,
        maximum_native_image_bound=maximum_native,maximum_archive_bound=maximum_archive)


def report():
    receipt=json.loads((PROOF/'receipts.json').read_bytes())
    required=['crates/viboceros-geometry/src/'+p for p in [
        'surface_fit/adaptive.rs','surface_fit/image.rs','surface_fit/image/certificate.rs',
        'surface_fit/image/correspondence.rs','surface_fit/image/correspondence/preflight.rs']]
    assert set(required).issubset(receipt['source_overrides'])
    for name,entry in receipt['source_overrides'].items(): assert digest(read(ROOT/entry['snapshot'])) == entry['sha256'],name
    for name,sha in receipt['files'].items(): assert digest((ROOT/name).read_bytes()) == sha,name
    checks=receipt['checks']; assert all(e['exit_code']==0 and e['log'] in receipt['files'] for e in checks.values())
    assert set(checks)=={'portable','native','uv_portable','uv_native','kernel','mapped_json','json_portable','json_native','app','app_native','format'}
    for name in ['uv_portable','uv_native','mapped_json','json_portable','json_native']:
        assert 'test result: ok. 1 passed; 0 failed;' in read(ROOT/checks[name]['log']).decode()
    for check in checks.values():
        for entry in check.get('source_overrides',{}).values():assert digest(read(ROOT/entry['snapshot']))==entry['sha256']
    assert '55 passed; 0 failed;' in read(ROOT/checks['kernel']['log']).decode()
    assert all('Finished' in read(ROOT/checks[k]['log']).decode() for k in ['app','app_native'])
    before=receipt['uniform_preflight_baseline']; assert before['exit_code']==0
    for entry in before['source_overrides'].values():assert digest(read(ROOT/entry['snapshot']))==entry['sha256']
    earlier=read(ROOT/before['log']).decode()
    assert 'test result: ok. 2 passed; 0 failed;' in earlier
    old_rows=rows(earlier,'SURFACE_IMAGE_FIT');current_rows=rows(read(ROOT/checks['portable']['log']).decode(),'SURFACE_IMAGE_FIT')
    assert len(old_rows)==len(current_rows)==112 and old_rows==current_rows
    assert len(receipt['unqualified_runs'])==5 and all(e['qualified']is False and e['exit_code']==101 for e in receipt['unqualified_runs'])
    captures=data()
    for family,(query,native) in zip(FAMILIES,captures):
        assert native['provenance']['request_sha256'] == digest((ROOT/('tools/rhino_oracle/fixtures/'+family+'.json')).read_bytes())
        for sha in native['provenance']['sources'].values():
            assert digest(read(ROOT/('tools/rhino_oracle/observations/source_snapshots/'+sha+'.gz'))) == sha
    assert receipt['new_rhino_launches']==0
    return dict(full_rhino_fit_parity=False,fit_srf_registered=False,normalized_uv_contract_changed=False,
        observations=112,native_surfaces=90,native_null=22,native_output_gate=1e-9,new_rhino_launches=0,
        knot_preflight_preserves_uniform_replay_outputs=True,
        backends={k:dict(uv=uv_summary(captures,read(ROOT/checks['uv_'+k]['log']).decode()),
            image=image_summary(captures,read(ROOT/checks[k]['log']).decode(),read(ROOT/receipt['baselines'][k]).decode())) for k in ['portable','native']},
        baseline_revision=receipt['baseline_revision'],receipts='surface-image-auto-proof/receipts.json')


if __name__=='__main__': print(json.dumps(report(),indent=2,allow_nan=False))
