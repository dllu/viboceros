"""Audit complete Ribbon corner images, topology and frozen native producers."""
import json
import math
from .report_surface_fit_basis import ROOT, read, digest
from .report_curve_loose_offset import settings

PROOF = ROOT / 'docs/ribbon-corner-proof'
FAMILIES = dict(ribbon_polyline=52, ribbon_corner_correspondence=29,
    ribbon_smooth_angles=11, ribbon_sharp_holdouts=12, ribbon_sharp_seams=6)
CAPTURES = list(FAMILIES)[1:]


def bound(value):
    assert type(value) in (int, float) and math.isfinite(value) and 0. <= value <= 1e-9
    return value


def summarize(corpus, log):
    marker = 'RIBBON_CORNERS '
    rows = [json.loads(s.split(marker, 1)[1]) for s in log.splitlines() if marker in s]
    assert len(rows) == len({(r['family'], r['id']) for r in rows}) == 110
    assert 'test result: ok. 3 passed; 0 failed;' in log
    assert 'test result: ok. 4 passed; 0 failed;' in log
    image = incidence = archive = 0.
    family_counts = {}; flips = []; orientations = 0; observational_offsets = 0
    for family, (query, native) in corpus.items():
        expected = FAMILIES[family]
        observed = [r for r in rows if r['family'] == family]
        selected = [(op, result) for op, result in zip(query['operations'], native['results']) if op['op'] == 'ribbon_command']
        assert len(observed) == len(selected) == expected
        assert [r['id'] for r in observed] == [o['id'] for o, _ in selected]
        for op, capture in zip(query['operations'], native['results']):
            assert op['id'] == capture['id']
            v = capture['value']; settings(op, v)
            assert v['success'] is True and v['returned_idle'] is True
            assert [o for o in v['before'] if o['source']] == [o for o in v['after'] if o['source']]
            observational_offsets += op['op'] == 'loose_offset_command'
        for (op, capture), row in zip(selected, observed):
            outputs = [o for o in capture['value']['created'] if not o['source']]
            assert len(outputs) == 1 and outputs[0]['valid'] is True
            g = outputs[0]['brep']['geometry']
            assert row['full_ribbon_command'] is False
            assert row['counts'] == [len(g['faces']), len(g['edges']), len(g['vertices'])]
            assert row['native_faces'] == len(g['faces']) == len(row['matches'])
            assert {m['native_index'] for m in row['matches']} == set(range(len(g['faces'])))
            for m in row['matches']: bound(m['bound'])
            image = max(image, bound(row['support_coverage_bound']))
            incidence = max(incidence, bound(row['incidence_bound']))
            archive = max(archive, bound(row['archive_bound']))
            assert type(row['native_face_orientation_equal']) is bool
            if row['native_face_orientation_equal']: orientations += 1
            else: flips.append(dict(family=family, id=op['id']))
        family_counts[family] = expected
    assert observational_offsets == 19
    return dict(complete_image_matches=110, incidence_matches=110,
        archive_support_round_trips=110, family_counts=family_counts,
        face_reversal_matches=orientations, face_reversal_difference_ids=flips,
        maximum_image_bound=image, maximum_incidence_geometry_bound=incidence,
        maximum_archive_support_bound=archive, native_offset_workflows_observational_only=19)


def report():
    r = json.loads((PROOF / 'receipts.json').read_bytes())
    for name, e in r['source_overrides'].items():
        assert digest(read(ROOT / e['snapshot'])) == e['sha256'], name
    for name, sha in r['files'].items(): assert digest((ROOT / name).read_bytes()) == sha, name
    checks = r['checks']
    required = {'portable', 'native', 'kernel', 'identities', 'offsets', 'join', 'app', 'app_native', 'format', 'probe'}
    required.update('capture_' + f for f in CAPTURES)
    assert set(checks) == required
    assert all(e['exit_code'] == 0 and e['log'] in r['files'] for e in checks.values())
    logs = {k: read(ROOT / e['log']).decode() for k, e in checks.items()}
    for k, count in [('kernel', 5), ('identities', 3), ('offsets', 99)]:
        assert 'test result: ok. '+str(count)+' passed; 0 failed;' in logs[k]
    assert 'test result: ok. 31 passed; 0 failed;' in logs['join']
    assert all('Finished `dev` profile' in logs[k] for k in ['app', 'app_native'])
    assert 'Ran 3 tests' in logs['probe']
    corpus = {}
    for family in FAMILIES:
        qpath = ROOT / ('tools/rhino_oracle/fixtures/' + family + '.json')
        q = json.loads(qpath.read_bytes())
        n = json.loads(read(ROOT / ('tools/rhino_oracle/observations/' + family + '.json.gz')))
        assert len(q['operations']) == len(n['results'])
        assert n['provenance']['request_sha256'] == digest(qpath.read_bytes())
        for sha in n['provenance']['sources'].values():
            assert digest(read(ROOT / ('tools/rhino_oracle/observations/source_snapshots/' + sha + '.gz'))) == sha
        corpus[family] = q, n
        if family in CAPTURES:
            key = 'capture_' + family
            assert checks[key]['command'].startswith('tools/rhino_oracle/run_headless.sh exec ')
            assert 'Recorded ' + family in logs[key]
    assert len(r['unqualified_runs']) >= 4
    assert all(e['exit_code'] == 101 and e['log'] in r['files'] for e in r['unqualified_runs'])
    return dict(baseline_revision=r['baseline_revision'], new_headless_rhino_launches=4,
        new_native_workflows=77, full_ribbon_command=False, ribbon_registered=False,
        smooth_polyline_corner_implemented=True, declared_vertex_identities_only=True,
        loop_winding_compared=False, output_domains_compared=False, between_station_distance_certified=False,
        backends={k: summarize(corpus, logs[k]) for k in ['portable', 'native']},
        existing_loose_ribbon_regressions=True, existing_curve_offset_regressions=True,
        receipts='ribbon-corner-proof/receipts.json')


if __name__ == '__main__': print(json.dumps(report(), indent=2, allow_nan=False))
