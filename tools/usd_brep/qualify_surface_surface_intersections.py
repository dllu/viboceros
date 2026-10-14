"""Record native surface/surface ABI, ASan and complete saved command comparisons."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
from tools.rhino_oracle import OracleClient, load_request
from tools.rhino_oracle.audit import compare_audit_response
from tools.rhino_oracle.client import _compare_value

ROOT = Path(__file__).resolve().parents[2]


def probe(path, sanitize=False):
    env = dict(os.environ)
    if sanitize:
        env['ASAN_OPTIONS'] = 'detect_leaks=0'
    result = subprocess.run([str(path.resolve())], capture_output=True, text=True,
                            env=env, timeout=120, check=False)
    response = json.loads(result.stdout)
    passed = (result.returncode == 0 and response.get('passed') is True
              and len(response['cases']) == 42 and all(c['passed'] for c in response['cases']))
    return dict(return_code=result.returncode, stderr=result.stderr, response=response,
                passed=passed, binary_sha256=hashlib.sha256(path.read_bytes()).hexdigest())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    parser.add_argument('--probe', type=Path, required=True)
    parser.add_argument('--asan', type=Path, required=True)
    args = parser.parse_args()
    fixture = 'tools/rhino_oracle/fixtures/native_curve_segment_commands.json'
    observation = 'tools/rhino_oracle/observations/native_curve_segment_commands.json'
    sources = [fixture, observation,
        'tools/usd_brep/qualify_surface_surface_intersections.py', 'tools/usd_brep/surface_surface_adapter_probe.cpp', 'tools/usd_brep/surface_surface_probe.cpp',
        'tools/usd_brep/CMakeLists.txt', 'crates/viboceros-smlib/native/bridge.cpp',
        'crates/viboceros-smlib/native/bridge.h', 'crates/viboceros-smlib/native/brep_import.cpp',
        'crates/viboceros-smlib/native/brep_data.h', 'crates/viboceros-smlib/native/brep_export.cpp', 'crates/viboceros-smlib/src/native/working_curve.rs',
        'crates/viboceros-smlib/src/native/curve_surface_intersections.rs',
        'crates/viboceros-smlib/src/native/knot_refinement.rs',
        'crates/viboceros-smlib/src/native/surface_intersections.rs', 'crates/viboceros-smlib/tests/surface_intersections.rs',
        'crates/viboceros-geometry/src/intersection.rs', 'crates/viboceros-geometry/src/lib.rs',
        'crates/viboceros-geometry/src/intersection/surface_surface_backend.rs',
        'crates/viboceros-geometry/src/intersection/surface_surface_backend/tests.rs', 'crates/viboceros-geometry/src/surface_pullback/certificate.rs',
        'crates/viboceros-command/src/lib.rs', 'crates/viboceros-command/src/curve_intersections.rs',
        'crates/viboceros-command/src/curve_intersections/tests.rs',
    ]
    source_hashes = {p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in sources}
    jobs = dict(native=probe(args.probe), address_sanitizer=probe(args.asan, True))
    request = load_request(ROOT/fixture)
    expected = json.loads((ROOT/observation).read_text())
    assert len(request['operations']) == 121
    native = OracleClient(native_smlib=True).run_viboceros_audit(request, timeout=1800)
    portable = OracleClient().run_viboceros_audit(request, timeout=1800)
    rhino = compare_audit_response(native, expected, 1e-8, 1e-12).as_dict()
    def rows(response):
        return {r['result']['id'] if r['status'] == 'success' else r['id']: r for r in response['outcomes']}
    a, b = rows(native), rows(portable)
    assert list(a) == list(b) == [o['id'] for o in request['operations']]
    producer = []
    for key in a:
        left, right = a[key], b[key]
        if left['status'] == 'failure' or right['status'] == 'failure':
            producer.append(dict(id=key, passed=False, native=left, portable=right))
            continue
        differences = []
        error = _compare_value(key, left['result']['value'], right['result']['value'], 1e-8, 1e-12, differences)
        producer.append(dict(id=key, passed=not differences, max_absolute_error=error, differences=differences))
    if source_hashes != {p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in sources}:
        raise ValueError('qualification sources changed while the jobs were running')
    passed = all(j['passed'] for j in jobs.values()) and rhino['passed'] and all(c['passed'] for c in producer)
    record = dict(upstream_commit='c9c5979f04cea8c38213004d4f04df0dcfb40d05', passed=passed,
        scope='42 actual C ABI cases per normal/ASan job: transverse, empty, coincident-boundary, corner-touch, '
              'saddle crossing, hyperbolic and degree-2-by-3 polynomial pairs; independent source charts and three planes. '
              'Sampled independent image formulas, unchanged inputs, transfer-once XYZ/UV snapshot ownership and invalid inputs. '
              'Rust separately requires continuous correspondence certificates on both original surfaces. '
              'Native command dispatch extends previously unsupported surface and full-natural-domain face pairs; '
              'general partial trims remain explicitly unsupported. '
              'All 121 saved intersection/cutting command records replayed at absolute 1e-8, relative 1e-12; '
              'same-request portable/native values retained separately. No fresh Rhino capture, general topology/completeness or speed claim.',
        sanitizer_scope='Complete SMLib, SM_API and C bridge; process-global context leak accounting alone disabled.',
        source_sha256=source_hashes,
        jobs=jobs, request=request, native_response=native, portable_response=portable,
        rhino_comparison=rhino, producer_comparison=producer)
    args.output.write_text(json.dumps(record, indent=2, allow_nan=False)+'\n')
    print('Surface/surface qualification:', passed, 'SDK matches:', rhino['matched'], '/', rhino['cases'], 'native errors:', rhino['native_failed'])
    if not passed:
        print('Failures:', [c['id'] for c in rhino['operations'] if not c['passed']])
        raise ValueError('qualification failed; all outcomes retained')


if __name__ == '__main__':
    main()
