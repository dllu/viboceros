"""Audit complete mapped image bounds separately from native output agreement."""
import json
import math
from .report_surface_fit_basis import ROOT, read, digest

PROOF = ROOT / 'docs/surface-mapped-proof'


def rows(log, prefix):
    return [json.loads(s[len(prefix)+1:]) for s in log.splitlines() if s.startswith(prefix+' ')]


def bound(value, limit):
    assert type(value) in (int, float) and math.isfinite(value) and 0. <= value <= limit
    return value


def summarize(query, native, log):
    observed = rows(log, 'MAPPED_FIT')
    operations, captured = query['operations'], native['results']
    assert len(observed) == len({r['id'] for r in observed}) == len(operations) == len(captured) == 15
    assert [r['id'] for r in observed] == [o['id'] for o in operations] == [r['id'] for r in captured]
    totals = dict(parameter=0, mapped=0, unproved=0, native_null=0)
    mapped_ids, unproved_ids = [], []
    for op, capture, row in zip(operations, captured, observed):
        value = capture['value']; assert value['before'] == value['after']
        if value['surface'] is None:
            assert row == dict(id=op['id'], native_null=True)
            totals['native_null'] += 1; continue
        limit = op['fit_tolerance']
        assert row['native_null'] is False and row['native_output_parity'] is False and row['limit'] == limit
        parameter, mapped = row['parameter_bound'], row['mapped_bound']
        if parameter is not None:
            bound(parameter, limit); assert mapped is row['inverse_bound'] is row['archive_bound'] is row['correspondence'] is row['map_stations'] is None
            totals['parameter'] += 1
        elif mapped is not None:
            for key in ['mapped_bound', 'inverse_bound', 'archive_bound']: bound(row[key], limit)
            maps = row['correspondence']; assert len(maps) == 2
            assert row['map_stations'] == list(map(len, maps))
            for pairs in maps:
                assert 2 <= len(pairs) <= 4097 and pairs[0] == [0., 0.] and pairs[-1] == [1., 1.]
                assert all(len(p) == 2 and all(type(t) in (int, float) and math.isfinite(t) for t in p) for p in pairs)
                assert all(all(a[k] < b[k] for k in [0, 1]) for a, b in zip(pairs, pairs[1:]))
            totals['mapped'] += 1; mapped_ids.append(row['id'])
        else:
            assert row['inverse_bound'] is row['archive_bound'] is row['correspondence'] is row['map_stations'] is None
            totals['unproved'] += 1; unproved_ids.append(row['id'])
    assert totals['parameter'] == 6 and totals['native_null'] == 1
    assert rows(log, 'MAPPED_FIT_TOTAL') == [totals]
    fitted = rows(log, 'MAPPED_BASIS'); assert [r['swapped'] for r in fitted] == [False, True]
    for row in fitted:
        assert row['limit'] == .001 and row['independently_supplied_basis'] is True and row['automatic_knot_parity'] is False
        bound(row['image_bound'], .001); bound(row['native_bound'], 1e-9); bound(row['archive_bound'], 1e-9)
    assert 'test result: ok. 3 passed; 0 failed;' in log
    return dict(**totals, mapped_ids=mapped_ids, unproved_ids=unproved_ids,
        fixed_basis_charts=2, maximum_native_output_bound=max(r['native_bound'] for r in fitted),
        maximum_archive_bound=max(r['archive_bound'] for r in fitted))


def report():
    receipt = json.loads((PROOF / 'receipts.json').read_bytes())
    required = ['crates/viboceros-geometry/src/'+p for p in [
        'surface_pullback/certificate/correspondence.rs',
        'surface_pullback/certificate/surface_pair/mapped.rs',
        'surface_pullback/certificate/interval/tensor.rs', 'surface_fit/image/basis.rs']]
    assert set(required).issubset(receipt['source_overrides'])
    for name, entry in receipt['source_overrides'].items():
        assert digest(read(ROOT / entry['snapshot'])) == entry['sha256'], name
    for name, sha in receipt['files'].items(): assert digest((ROOT / name).read_bytes()) == sha, name
    checks = receipt['checks']
    assert set(checks) == {'portable', 'native', 'certificates', 'image_kernel', 'g1', 'app', 'app_native', 'format'}
    assert all(e['exit_code'] == 0 and e['log'] in receipt['files'] for e in checks.values())
    for name, count in [('certificates',149),('image_kernel',11)]:
        assert 'test result: ok. %d passed; 0 failed;' % count in read(ROOT/checks[name]['log']).decode()
    assert '19 passed; 0 failed; 1 ignored;' in read(ROOT/checks['g1']['log']).decode()
    fixture = 'tools/rhino_oracle/fixtures/surface_fit_achieved.json'
    observation = 'tools/rhino_oracle/observations/surface_fit_achieved.json.gz'
    query_bytes = (ROOT/fixture).read_bytes(); query = json.loads(query_bytes); native = json.loads(read(ROOT/observation))
    assert fixture in receipt['files'] and observation in receipt['files']
    assert native['provenance']['request_sha256'] == digest(query_bytes)
    for sha in native['provenance']['sources'].values():
        assert digest(read(ROOT/('tools/rhino_oracle/observations/source_snapshots/'+sha+'.gz'))) == sha
    assert receipt['new_rhino_launches'] == 0
    assert len(receipt['unqualified_runs']) == 3
    assert all(e['exit_code'] == 101 and e['qualified'] is False and e['log'] in receipt['files']
        for e in receipt['unqualified_runs'])
    return dict(full_rhino_fit_parity=False, fit_srf_registered=False, automatic_fitter_admission_changed=False,
        source_image_gate='requested fit tolerance', native_output_gate=1e-9, new_rhino_launches=0,
        backends={k:summarize(query,native,read(ROOT/checks[k]['log']).decode()) for k in ['portable','native']},
        baseline_revision=receipt['baseline_revision'], receipts='surface-mapped-proof/receipts.json')


if __name__ == '__main__': print(json.dumps(report(), indent=2, allow_nan=False))
