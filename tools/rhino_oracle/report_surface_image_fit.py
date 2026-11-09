"""Audit new image fits alongside unchanged normalized-UV fitting results."""
import collections
import json
from . import report_surface_fit_adaptive as adaptive
from .report_surface_fit_basis import ROOT, digest, read

PROOF = ROOT / 'docs/surface-image-fit-proof'
SOURCE_OVERRIDES = ['crates/viboceros-geometry/src/' + name for name in
                    ['lib.rs', 'surface_fit.rs', 'surface_fit/adaptive.rs',
                     'surface_fit/image.rs', 'surface_fit/image/tests.rs']] + [
    'crates/viboceros-smlib/src/' + name for name in
    ['lib.rs', 'native.rs', 'native/knot_refinement.rs', 'native/knot_refinement/interpolation.rs']
] + ['crates/viboceros-oracle/' + name for name in
     ['src/lib.rs', 'src/surface_fit_bounded.rs', 'tests/surface_fit_adaptive_native.rs',
      'tests/surface_fit_image_native.rs']] + [
    'tools/rhino_oracle/' + name for name in
    ['report_surface_image_fit.py', 'test_surface_image_fit_record.py']
]


def valid_bound(value, limit):
    if value is not None:
        assert type(value) in (int, float) and 0. <= value <= limit
    return value


def geometric_bound(parameter, planar):
    values = [valid_bound(v, 1e-9) for v in [parameter, planar] if v is not None]
    return min(values) if values else None


def summarize(captures, log):
    # This independently audits all previous UV and captured-basis rows too.
    legacy = adaptive.summarize(captures, log)
    assert legacy['automatic_counts'] == dict(native_match=35, native_correspondence_unproved=40,
        native_null_with_fit=12, native_surface_without_fit=15, both_declined=10)
    assert legacy['certified_outputs'] == 87
    uv = adaptive.rows(log, 'AUTOMATIC_SURFACE_FIT')
    image = adaptive.rows(log, 'SURFACE_IMAGE_FIT')
    operations = [o for q, _ in captures for o in q['operations']]
    native = [r for _, n in captures for r in n['results']]
    assert len(image) == len({r['id'] for r in image}) == 112
    assert [r['id'] for r in image] == [o['id'] for o in operations]
    assert 'test result: ok. 2 passed; 0 failed;' in log
    counts = collections.Counter()
    planar = added = reduced = uv_image_matches = 0
    maximum = maximum_archive = maximum_uv_image = 0.
    for op, native, previous, row in zip(operations, native, uv, image):
        value = native['value']
        exists = value['surface'] is not None
        flat = all(p['point'][2] == 0. for p in value['before']['control_points'])
        assert value['before'] == value['after']
        assert row['planar_fixture'] is flat
        assert row['maximum_controls'] == [16, 16] and row['limit'] == op['fit_tolerance']
        old_bound = geometric_bound(previous['native_bound'], previous['native_planar_bound'])
        if old_bound is not None:
            assert previous['source_bound'] is not None and exists
            uv_image_matches += 1
            maximum_uv_image = max(maximum_uv_image, old_bound)
        fitted = row['source_bound'] is not None
        if not flat:
            assert row['controls'] == previous['controls']
            assert row['source_bound'] == previous['source_bound']
        bound = geometric_bound(row['native_parameter_bound'], row['native_planar_bound'])
        if fitted:
            valid_bound(row['source_bound'], row['limit'])
            assert all(op['degree_'+a]+1 <= n <= 16 for a, n in zip(['u', 'v'], row['controls']))
            assert len(row['controls']) == 2
            assert row['archive_bound'] is not None
            maximum_archive = max(maximum_archive, valid_bound(row['archive_bound'], 1e-9))
            added += previous['source_bound'] is None
            if previous['controls'] is not None:
                reduced += row['controls'][0]*row['controls'][1] < previous['controls'][0]*previous['controls'][1]
            if bound is not None:
                assert exists
                maximum = max(maximum, bound)
            status = ('native_image_match' if bound is not None else 'native_image_unproved') if exists else 'native_null_with_fit'
        else:
            assert row['controls'] is row['archive_bound'] is row['native_parameter_bound'] is row['native_planar_bound'] is None
            assert previous['source_bound'] is None
            status = 'native_surface_without_fit' if exists else 'both_declined'
        if not exists:
            assert bound is None
        if flat:
            assert fitted and exists and row['native_planar_bound'] is not None
            assert row['controls'] == [op['degree_u']+1, op['degree_v']+1]
            planar += 1
        assert row['status'] == status
        counts[status] += 1
    assert planar == 30
    assert (added, reduced, uv_image_matches) == (6, 13, 48)
    assert counts == dict(native_image_match=54, native_image_unproved=27,
        native_null_with_fit=12, native_surface_without_fit=9, both_declined=10)
    assert adaptive.rows(log, 'SURFACE_IMAGE_FIT_TOTAL') == [dict(cases=112, counts=dict(counts),
        planar_native_matches=30, full_rhino_fit_parity=False)]
    return dict(normalized_uv_fitter=legacy,
        normalized_uv_fitter_image_comparison=dict(native_image_matches=uv_image_matches,
            maximum_native_image_bound=maximum_uv_image),
        image_fitter=dict(counts=dict(counts), certified_outputs=sum(counts[k] for k in
            ['native_image_match', 'native_image_unproved', 'native_null_with_fit']),
            newly_fitted=added, reduced_control_counts=reduced, planar_single_patch_native_matches=30,
            maximum_native_image_bound=maximum, maximum_archive_bound=maximum_archive))


def report():
    receipt = json.loads((PROOF / 'receipts.json').read_bytes())
    assert set(receipt['source_overrides']) == set(SOURCE_OVERRIDES)
    for name, entry in receipt['source_overrides'].items():
        assert digest(read(ROOT / entry['snapshot'])) == entry['sha256'], name
    checks = receipt['checks']
    assert set(checks) == {'portable', 'native', 'kernel', 'app', 'format', 'python'}
    assert all(c['exit_code'] == 0 and c['log'] in receipt['files'] for c in checks.values())
    required = {c['log'] for c in checks.values()}
    for family in adaptive.FAMILIES:
        required.update('tools/rhino_oracle/'+directory+'/'+family+suffix
                        for directory, suffix in [('fixtures', '.json'), ('observations', '.json.gz')])
    assert required.issubset(receipt['files'])
    for name, sha in receipt['files'].items():
        assert digest((ROOT / name).read_bytes()) == sha, name
    assert 'test result: ok. 46 passed; 0 failed;' in read(ROOT / checks['kernel']['log']).decode()
    assert 'Ran 13 tests' in read(ROOT / checks['python']['log']).decode()
    assert 'Finished' in read(ROOT / checks['app']['log']).decode()
    captures = adaptive.data()
    return dict(full_rhino_fit_parity=False, fit_srf_registered=False,
        normalized_uv_contract_changed=False, native_comparison_tolerance=1e-9,
        observations=112, native_surfaces=90, native_null=22, maximum_controls=[16, 16],
        backends={key: summarize(captures, read(ROOT / checks[key]['log']).decode())
                  for key in ['portable', 'native']},
        baseline_revision=receipt['baseline_revision'], receipts='surface-image-fit-proof/receipts.json')


if __name__ == '__main__':
    print(json.dumps(report(), indent=2, allow_nan=False))
