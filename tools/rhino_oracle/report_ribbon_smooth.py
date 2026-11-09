"""Audit frozen ordinary smooth Ribbon images and explicit certificate limits."""
import json
import math
from .report_surface_fit_basis import ROOT, read, digest
from .report_curve_loose_offset import settings

PROOF = ROOT / 'docs/ribbon-smooth-proof'
FAMILIES = dict(ribbon_options=7, ribbon_smooth=37, ribbon_analytic=8, ribbon_near_analytic=8)
CAPTURES = list(FAMILIES)[1:]
OPTION_IDS = {'line', 'line-both', 'line-input-layer', 'circle-outside',
    'circle-inside', 'circle-both', 'smooth'}


def bound(value):
    assert type(value) in (int, float) and math.isfinite(value) and 0. <= value <= 1e-9
    return value


def summarize(corpus, log):
    marker = 'RIBBON_SMOOTH '
    rows = [json.loads(s.split(marker, 1)[1]) for s in log.splitlines() if marker in s]
    assert len(rows) == len({(r['family'], r['id']) for r in rows}) == 60
    for count in [2, 3, 4]:
        assert 'test result: ok. '+str(count)+' passed; 0 failed;' in log
    assert log.count('RIBBON_CORNERS ') == 110 and log.count('RIBBON_RULED ') == 32
    image = incidence = archive = 0.
    times = {}
    for family, (query, native) in corpus.items():
        selected = [(o, r) for o, r in zip(query['operations'], native['results'])
            if family != 'ribbon_options' or o['id'] in OPTION_IDS]
        observed = [r for r in rows if r['family'] == family]
        assert len(selected) == len(observed) == FAMILIES[family]
        assert [r['id'] for r in observed] == [o['id'] for o, _ in selected]
        for (op, capture), row in zip(selected, observed):
            assert op['id'] == capture['id']
            v = capture['value']; settings(op, v)
            assert v['success'] is True and v['returned_idle'] is True
            assert [o for o in v['before'] if o['source']] == [o for o in v['after'] if o['source']]
            outputs = [o for o in v['created'] if not o['source']]
            assert len(outputs) == 1 and outputs[0]['valid'] is True
            g = outputs[0]['brep']['geometry']
            assert row['error'] is None
            assert row['full_ribbon_command'] is False and row['distance_certified'] is False
            assert row['counts'] == [len(g['faces']), len(g['edges']), len(g['vertices'])]
            assert row['native_faces'] == len(g['faces']) == len(row['matches'])
            assert {m['native_index'] for m in row['matches']} == set(range(len(g['faces'])))
            for m in row['matches']: bound(m['bound'])
            image = max(image, bound(row['support_coverage_bound']))
            incidence = max(incidence, bound(row['incidence_bound']))
            archive = max(archive, bound(row['archive_bound']))
            for key in ['construction_ns', 'verification_ns']:
                assert type(row[key]) is int and row[key] >= 0
            times[family+'/'+op['id']] = {k: row[k] for k in ['construction_ns', 'verification_ns']}
    return dict(complete_image_matches=60, incidence_matches=60,
        archive_support_round_trips=60, family_counts=FAMILIES,
        maximum_image_bound=image, maximum_incidence_geometry_bound=incidence,
        maximum_archive_support_bound=archive,
        rational_both_diagnostic_timings_ns=times['ribbon_smooth/rational-both'],
        maximum_construction_ns=max(t['construction_ns'] for t in times.values()),
        existing_polyline_replays=110, existing_loose_replays=32)


def report():
    r = json.loads((PROOF / 'receipts.json').read_bytes())
    for name, e in r['source_overrides'].items():
        assert digest(read(ROOT / e['snapshot'])) == e['sha256'], name
    assert r['source_deletions'] == ['crates/viboceros-oracle/src/ribbon_loose.rs']
    for name, sha in r['files'].items(): assert digest((ROOT / name).read_bytes()) == sha, name
    required = {'portable', 'native', 'kernel', 'image', 'offsets', 'join', 'app', 'app_native', 'format', 'probe'}
    required.update('capture_'+f for f in CAPTURES)
    checks = r['checks']; assert set(checks) == required
    assert all(e['exit_code'] == 0 and e['log'] in r['files'] for e in checks.values())
    logs = {k: read(ROOT / e['log']).decode() for k, e in checks.items()}
    for key, count in [('kernel', 3), ('image', 3), ('offsets', 99), ('join', 33)]:
        assert 'test result: ok. '+str(count)+' passed; 0 failed;' in logs[key]
    assert all('Finished `dev` profile' in logs[k] for k in ['app', 'app_native'])
    assert 'Ran 3 tests' in logs['probe']
    corpus = {}
    for family in FAMILIES:
        qpath = ROOT / ('tools/rhino_oracle/fixtures/'+family+'.json')
        q = json.loads(qpath.read_bytes())
        n = json.loads(read(ROOT / ('tools/rhino_oracle/observations/'+family+'.json.gz')))
        assert len(q['operations']) == len(n['results'])
        assert n['provenance']['request_sha256'] == digest(qpath.read_bytes())
        for sha in n['provenance']['sources'].values():
            assert digest(read(ROOT / ('tools/rhino_oracle/observations/source_snapshots/'+sha+'.gz'))) == sha
        corpus[family] = q, n
        if family in CAPTURES:
            key = 'capture_'+family
            assert checks[key]['command'].startswith('tools/rhino_oracle/run_headless.sh exec ')
            assert 'Recorded '+family in logs[key]
    assert all(e['log'] in r['files'] and e['qualified'] is False for e in r['unqualified_runs'])
    return dict(baseline_revision=r['baseline_revision'], new_headless_rhino_launches=3,
        new_native_workflows=53, full_ribbon_command=False, ribbon_registered=False,
        between_station_distance_certified=False, u_correspondence_certified=False,
        native_performance_parity=False, loop_winding_compared=False,
        output_domains_compared=False, independent_source_gauges_preserved=True,
        backends={k: summarize(corpus, logs[k]) for k in ['portable', 'native']},
        receipts='ribbon-smooth-proof/receipts.json')


if __name__ == '__main__': print(json.dumps(report(), indent=2, allow_nan=False))
