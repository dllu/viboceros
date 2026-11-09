"""Audit ordinary polyline Ribbon tensors without promoting known gaps."""
import json
import math
from .report_surface_fit_basis import ROOT, read, digest
from .report_curve_loose_offset import settings

PROOF = ROOT / 'docs/ribbon-polyline-proof'


def bound(value):
    assert type(value) in (int, float) and math.isfinite(value) and 0. <= value <= 1e-9
    return value


def summarize(query, native, log):
    rows = [json.loads(s.split(' ', 1)[1]) for s in log.splitlines() if s.startswith('RIBBON_POLYLINE ')]
    ids = [o['id'] for o in query['operations']]
    assert len(ids) == len(set(ids)) == len(native['results']) == len(rows) == 52
    assert ids == [r['id'] for r in rows] == [r['id'] for r in native['results']]
    assert 'test result: ok. 2 passed; 0 failed;' in log
    assert 'test result: ok. 4 passed; 0 failed;' in log
    image = incidence = archive = 0.
    images = incidences = orientations = 0
    image_gaps = []; point_gaps = []; flips = []
    for op, capture, row in zip(query['operations'], native['results'], rows):
        v = capture['value']; settings(op, v)
        assert v['success'] is True and v['returned_idle'] is True
        assert [o for o in v['before'] if o['source']] == [o for o in v['after'] if o['source']]
        output = [o for o in v['created'] if not o['source']]
        assert len(output) == 1
        g = output[0]['brep']['geometry']
        assert row['native_faces'] == len(g['faces']) and row['full_ribbon_command'] is False
        gap = op['corner'] == 'Sharp' and op['id'].startswith('zigzag')
        point_gap = op['corner'] == 'None' and not op['both_sides']
        if gap:
            assert row['support_coverage_bound'] is None and row['incidence_bound'] is None
            assert row['native_face_orientation_equal'] is None
            assert (row['counts'], len(g['faces'])) == (([6, 17, 12], 10) if op['both_sides'] else ([3, 10, 8], 5))
            image_gaps.append(op['id'])
        else:
            images += 1; image = max(image, bound(row['support_coverage_bound']))
            assert len(row['matches']) == len(g['faces'])
            assert {m['native_index'] for m in row['matches']} == set(range(len(g['faces'])))
            for m in row['matches']: bound(m['bound'])
            if point_gap:
                assert row['incidence_bound'] is None
                assert row['counts'][:2] == [len(g['faces']), len(g['edges'])]
                assert row['counts'][2] > len(g['vertices'])
                point_gaps.append(op['id'])
            else:
                incidences += 1; incidence = max(incidence, bound(row['incidence_bound']))
                assert row['counts'] == [len(g['faces']), len(g['edges']), len(g['vertices'])]
            if row['native_face_orientation_equal']: orientations += 1
            else: flips.append(op['id'])
        archive = max(archive, bound(row['archive_bound']))
    assert (images, incidences, orientations, len(point_gaps), len(flips)) == (50, 42, 31, 8, 19)
    assert image_gaps == ['zigzag-sharp', 'zigzag-both-sharp']
    assert all(op['both_sides'] for op in query['operations'] if op['id'] in flips)
    return dict(complete_image_matches=images, incidence_matches=incidences, face_reversal_matches=orientations,
        image_unproved_ids=image_gaps, point_incidence_gap_ids=point_gaps, face_reversal_difference_ids=flips,
        maximum_image_bound=image, maximum_incidence_geometry_bound=incidence, maximum_archive_support_bound=archive)


def report():
    receipt = json.loads((PROOF / 'receipts.json').read_bytes())
    for name, entry in receipt['source_overrides'].items():
        assert digest(read(ROOT / entry['snapshot'])) == entry['sha256'], name
    for name, sha in receipt['files'].items():
        assert digest((ROOT / name).read_bytes()) == sha, name
    checks = receipt['checks']
    assert set(checks) == {'portable', 'native', 'kernel', 'offset', 'app', 'app_native', 'format', 'capture', 'probe'}
    assert all(e['exit_code'] == 0 and e['log'] in receipt['files'] for e in checks.values())
    assert checks['capture']['command'].startswith('tools/rhino_oracle/run_headless.sh exec ')
    logs = {k: read(ROOT / e['log']).decode() for k, e in checks.items()}
    assert 'Recorded ribbon_polyline' in logs['capture']
    assert 'test result: ok. 4 passed; 0 failed;' in logs['kernel']
    assert 'test result: ok. 99 passed; 0 failed;' in logs['offset']
    assert 'Ran 3 tests' in logs['probe']
    assert all('Finished `dev` profile' in logs[k] for k in ['app', 'app_native'])
    qpath = ROOT / 'tools/rhino_oracle/fixtures/ribbon_polyline.json'
    query = json.loads(qpath.read_bytes())
    native = json.loads(read(ROOT / 'tools/rhino_oracle/observations/ribbon_polyline.json.gz'))
    assert native['provenance']['request_sha256'] == digest(qpath.read_bytes())
    for sha in native['provenance']['sources'].values():
        assert digest(read(ROOT / ('tools/rhino_oracle/observations/source_snapshots/' + sha + '.gz'))) == sha
    assert len(receipt['unqualified_runs']) == 2
    assert all(e['exit_code'] == 101 and e['log'] in receipt['files'] for e in receipt['unqualified_runs'])
    return dict(baseline_revision=receipt['baseline_revision'], new_headless_rhino_launches=1,
        full_ribbon_command=False, ribbon_registered=False, smooth_corner_implemented=False,
        loop_winding_compared=False, output_domains_compared=False, between_station_distance_certified=False,
        backends={k: summarize(query, native, logs[k]) for k in ['portable', 'native']},
        existing_loose_ribbon_regressions=True, existing_curve_offset_regressions=True,
        receipts='ribbon-polyline-proof/receipts.json')


if __name__ == '__main__':
    print(json.dumps(report(), indent=2, allow_nan=False))
