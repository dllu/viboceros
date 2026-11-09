"""Audit fixed-plane Ribbon evidence with explicit native-success support gaps."""
import json
from .report_surface_fit_basis import ROOT, read, digest
from .report_curve_loose_offset import settings, provenance
from .report_ribbon_command import bound

PROOF = ROOT / 'docs/ribbon-cplane-proof'
FAMILIES = dict(ribbon_options=13, ribbon_workflows=12, ribbon_cplane=32,
                ribbon_cplane_boundaries=15, ribbon_cplane_policy=20,
                ribbon_cplane_switches=17, ribbon_cplane_thresholds=16,
                ribbon_cplane_holdouts=6)


def gap(family, name):
    if family == 'ribbon_cplane_boundaries':
        return name in ['parallel-tilt-0.001', 'parallel-tilt-1e-05', 'parallel-tilt-1e-07', 'parallel-tilt-1e-09', 'parallel-tilt-1e-11', 'linear-projection-0.0001']
    if family == 'ribbon_cplane_policy':
        return name in ['parallel-angle-0.005', 'parallel-angle-0.01', 'perpendicular-0.001', 'height-0.0001']
    if family == 'ribbon_cplane_switches':
        return name != 'perpendicular-0.008'
    if family == 'ribbon_cplane_thresholds':
        return True
    if family == 'ribbon_cplane_holdouts':
        return not name.startswith('cubic-')
    return False


def summarize(corpus, log):
    marker = 'RIBBON_COMMAND '
    rows = [json.loads(line.split(marker, 1)[1]) for line in log.splitlines() if marker in line]
    assert len(rows) == len({(r['family'], r['id']) for r in rows}) == sum(FAMILIES.values())
    assert 'test result: ok. 8 passed; 0 failed;' in log
    maximum = incidence = 0.
    matched = 0
    gaps = []
    for family, (query, native) in corpus.items():
        observed = [r for r in rows if r['family'] == family]
        assert len(query['operations']) == len(native['results']) == len(observed) == FAMILIES[family]
        for op, capture, row in zip(query['operations'], native['results'], observed):
            assert op['id'] == capture['id'] == row['id']
            value = capture['value']
            settings(op, value)
            assert value['success'] is True and value['returned_idle'] is True
            assert row['native_success'] is True and row['full_command_parity'] is False
            assert [o for o in value['before'] if o['source']] == [o for o in value['after'] if o['source']]
            outputs = [o for o in value['created'] if not o['source']]
            assert len(outputs) == 1 and outputs[0]['valid'] is True
            unsupported = gap(family, op['id'])
            assert row['local_supported'] is (not unsupported)
            if unsupported:
                reason = 'coarse_nonpolynomial_fit' if family == 'ribbon_cplane_holdouts' else 'unqualified_plane_switch'
                assert row['reason'] == reason and row['failure_atomic'] is True
                assert row['u_correspondence_compared'] is False
                gaps.append(dict(family=family, id=op['id'], reason=reason))
            else:
                assert row['attributes_selection_history'] is True
                assert row['u_correspondence_compared'] is (family in ['ribbon_options', 'ribbon_workflows'])
                maximum = max(maximum, bound(row['image_bound']))
                incidence = max(incidence, bound(row['incidence_bound']))
                matched += 1
    assert matched == 85 and len(gaps) == 46
    return dict(complete_image_and_incidence_matches=matched,
                attributes_selection_history_matches=matched,
                maximum_image_bound=maximum, maximum_incidence_bound=incidence,
                native_success_local_unsupported=gaps)


def corpus():
    result = {}
    for family in FAMILIES:
        fixture = ROOT / ('tools/rhino_oracle/fixtures/' + family + '.json')
        native = json.loads(read(ROOT / ('tools/rhino_oracle/observations/' + family + '.json.gz')))
        provenance(native, fixture)
        result[family] = json.loads(fixture.read_bytes()), native
    return result


def report():
    receipts = json.loads((PROOF / 'receipts.json').read_bytes())
    for name, entry in receipts['source_overrides'].items():
        assert digest(read(ROOT / entry['snapshot'])) == entry['sha256'], name
    for name, sha in receipts['files'].items():
        assert digest((ROOT / name).read_bytes()) == sha, name
    checks = receipts['checks']
    expected = {'portable', 'native', 'kernel', 'command', 'ui', 'ui_native', 'app', 'app_native', 'format', 'probe'}
    assert set(checks) == expected
    assert all(e['exit_code'] == 0 and e['log'] in receipts['files'] for e in checks.values())
    logs = {key: read(ROOT / entry['log']).decode() for key, entry in checks.items()}
    for key, count in [('kernel', 3), ('command', 4), ('ui', 3), ('ui_native', 3)]:
        assert 'test result: ok. ' + str(count) + ' passed; 0 failed;' in logs[key]
    assert all('Finished `dev` profile' in logs[key] for key in ['app', 'app_native'])
    assert 'Ran 4 tests' in logs['probe']
    captures = receipts['captures']
    assert len(captures) == 6
    for entry in captures:
        assert entry['exit_code'] == 0 and entry['log'] in receipts['files']
        assert entry['command'].startswith('tools/rhino_oracle/run_headless.sh exec ')
        assert 'Recorded ' + entry['family'] in read(ROOT / entry['log']).decode()
    assert all(e['qualified'] is False and e['log'] in receipts['files'] for e in receipts['unqualified_runs'])
    inputs = corpus()
    return dict(baseline_revision=receipts['baseline_revision'], full_command_parity=False,
                new_headless_rhino_launches=6, new_native_workflows=106,
                between_station_distance_certified=False, loop_winding_compared=False,
                fixed_plane_u_correspondence_compared=False, native_performance_parity=False,
                custom_tolerance_scope='qualified single-span polynomial cases only',
                backends={key: summarize(inputs, logs[key]) for key in ['portable', 'native']},
                receipts='ribbon-cplane-proof/receipts.json')


if __name__ == '__main__':
    print(json.dumps(report(), indent=2, allow_nan=False))
