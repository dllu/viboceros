"""Retain self-contact ABI checks and complete saved Rhino command/API diagnostics."""
import argparse
import copy
import hashlib
import json
import os
from pathlib import Path
import subprocess
from tools.rhino_oracle import OracleClient, load_request
from tools.rhino_oracle.client import _compare_value

ROOT = Path(__file__).resolve().parents[2]
BASIC = {'crossing', 'simple-closed', 'backtrack', 'cubic-crossing', 'cubic-seam'}


def probe(path, sanitize=False, count=36):
    env = dict(os.environ)
    if sanitize:
        env['ASAN_OPTIONS'] = 'detect_leaks=0'
    result = subprocess.run([str(path.resolve())], capture_output=True, text=True,
                            env=env, timeout=120, check=False)
    response = json.loads(result.stdout)
    passed = (result.returncode == 0 and response['passed'] is True
              and len(response['cases']) == count and all(c['passed'] for c in response['cases']))
    return dict(return_code=result.returncode, stderr=result.stderr, response=response,
                passed=passed, binary_sha256=hashlib.sha256(path.read_bytes()).hexdigest())


def compare(actual, expected, epsilon=1e-8):
    differences = []
    error = _compare_value('value', actual, expected, epsilon, 0., differences)
    return dict(passed=not differences, max_absolute_error=error, differences=differences)


def command_value(value):
    """Project command fields for diagnostics; raw API values remain in the record."""
    return {k: v for k, v in value.items() if k != 'api_events'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    parser.add_argument('--probe', type=Path, required=True)
    parser.add_argument('--asan', type=Path, required=True)
    parser.add_argument('--baseline', type=Path, required=True)
    parser.add_argument('--range-probe', type=Path, required=True)
    parser.add_argument('--asan-range', type=Path, required=True)
    parser.add_argument('--range-baseline', type=Path, required=True)
    args = parser.parse_args()
    jobs = dict(native=probe(args.probe), address_sanitizer=probe(args.asan, sanitize=True),
                native_ranges=probe(args.range_probe, count=7),
                address_sanitizer_ranges=probe(args.asan_range, sanitize=True, count=7))
    baseline = json.loads(args.baseline.read_text())
    sources = [
        'tools/usd_brep/qualify_curve_self_intersections.py',
        'tools/usd_brep/self_intersection_adapter_probe.cpp', 'tools/usd_brep/self_intersection_range_probe.cpp', 'tools/usd_brep/CMakeLists.txt',
        'crates/viboceros-smlib/native/bridge.cpp', 'crates/viboceros-smlib/native/bridge.h',
        'crates/viboceros-smlib/native/CMakeLists.txt', 'crates/viboceros-smlib/build.rs',
        'crates/viboceros-smlib/src/native/curve_intersections.rs',
        'crates/viboceros-smlib/src/native/working_curve.rs',
        'crates/viboceros-smlib/tests/curve_self_intersections.rs',
        'crates/viboceros-geometry/src/nurbs/intersection_backend.rs',
        'crates/viboceros-geometry/src/nurbs.rs',
        'crates/viboceros-geometry/src/nurbs/intersection_backend/tests.rs',
        'crates/viboceros-command/src/intersect_self.rs',
        'crates/viboceros-oracle/src/self_intersect.rs', 'crates/viboceros-oracle/src/lib.rs',
        'third_party/patches/usd-brep-curve-self-intersection-pairs.patch',
        'third_party/patches/usd-brep-curve-self-intersection-pairs.cmake',
        'third_party/patches/usd-brep-curve-intersection-distance.patch',
        'third_party/patches/usd-brep-curve-intersection-distance-solver.patch',
        'third_party/patches/usd-brep-curve-intersection-distance-header.patch',
        'third_party/patches/usd-brep-curve-intersection-distance.cmake',
    ]
    inputs = []
    native_request = dict(protocol_version=1, iterations=1, operations=[])
    portable_request = copy.deepcopy(native_request)
    for name in ('self_intersect', 'self_intersect_selection', 'self_intersect_overlaps'):
        fixture = f'tools/rhino_oracle/fixtures/{name}.json'
        observation = f'tools/rhino_oracle/observations/{name}.json'
        sources.extend((fixture, observation))
        request = load_request(ROOT / fixture)
        assert request['protocol_version'] == 1 and request['iterations'] == 1 and 'tolerance' not in request
        expected = json.loads((ROOT / observation).read_text())
        assert [o['id'] for o in request['operations']] == [r['id'] for r in expected['results']]
        inputs.append((name, request, expected))
        for original in request['operations']:
            op = copy.deepcopy(original)
            op['id'] = name + '/' + op['id']
            native_request['operations'].append(op)
            op = copy.deepcopy(op)
            op['include_api'] = False
            portable_request['operations'].append(op)
    native = OracleClient(native_smlib=True).run_viboceros_audit(native_request, timeout=1800)
    portable = OracleClient().run_viboceros_audit(portable_request, timeout=1800)
    def outcomes(response):
        return {r['result']['id'] if r['status'] == 'success' else r['id']: r for r in response['outcomes']}
    native_rows, portable_rows = outcomes(native), outcomes(portable)
    assert list(native_rows) == [o['id'] for o in native_request['operations']]
    assert list(portable_rows) == [o['id'] for o in portable_request['operations']]
    def checked(row, reference, command=False):
        if row['status'] == 'failure':
            return dict(passed=False, native_error=row['error'])
        value = row['result']['value']
        return compare(command_value(value) if command else value, reference)
    fixtures = []
    for name, request, expected in inputs:
        checks = []
        for reference in expected['results']:
            key = name + '/' + reference['id']
            a, b = native_rows[key], portable_rows[key]
            producer = (checked(a, b['result']['value'], command=True) if b['status'] == 'success'
                        else dict(passed=False, portable_error=b['error']))
            checks.append(dict(id=reference['id'], basic_gate=name != 'self_intersect_overlaps' and reference['id'] in BASIC,
                native_raw_rhino=checked(a, reference['value']),
                native_command_rhino=checked(a, command_value(reference['value']), command=True),
                portable_command_rhino=checked(b, command_value(reference['value'])),
                native_portable_command=producer))
        fixtures.append(dict(name=name, rhino_request=request, rhino_response=expected, checks=checks))
    gate = [c for f in fixtures for c in f['checks'] if c['basic_gate']]
    passed = all(j['passed'] for j in jobs.values()) and len(gate) == 10 and all(
        c['native_command_rhino']['passed'] and c['native_portable_command']['passed'] for c in gate)
    record = dict(upstream_commit='c9c5979f04cea8c38213004d4f04df0dcfb40d05', passed=passed,
        all_raw_rhino_passed=all(c['native_raw_rhino']['passed'] for f in fixtures for c in f['checks']),
        scope='36 independent analytic C ABI cases plus seven multi-visit range image checks per normal/ASan job. Ten saved basic command/selection cases form the parity gate. '
              'All 29 saved cases, including nested/backtracking overlap command and API discrepancies, are retained without a broad parity claim. '
              'Portable requests disable the newly exposed native-only paired-parameter API; every other input is unchanged. No fresh Rhino capture or speed claim.',
        sanitizer_scope='Complete SMLib, SM_API and C bridge; process-global context leak accounting alone disabled.',
        source_sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in sources},
        jobs=jobs, baseline=baseline, range_baseline=json.loads(args.range_baseline.read_text()), fixtures=fixtures,
        native_request=native_request, portable_request=portable_request, native_response=native, portable_response=portable)
    args.output.write_text(json.dumps(record, indent=2, allow_nan=False)+'\n')
    print('Analytic ABI and basic command gate:', passed)
    print('Complete raw Rhino parity:', record['all_raw_rhino_passed'])
    for f in fixtures:
        print(f['name'], 'raw matches', sum(c['native_raw_rhino']['passed'] for c in f['checks']), '/', len(f['checks']))
    if not passed:
        raise ValueError('self-intersection qualification gate failed; retained outcomes')


if __name__ == '__main__':
    main()
