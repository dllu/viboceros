"""Record native surface trim tangencies, ASan and complete saved command comparisons."""
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
              and len(response['cases']) == 24 and all(c['passed'] for c in response['cases'])
              and sum(c['contacts'] for c in response['cases']) > 0)
    return dict(return_code=result.returncode, stderr=result.stderr, response=response,
                passed=passed, binary_sha256=hashlib.sha256(path.read_bytes()).hexdigest())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    parser.add_argument('--probe', type=Path, required=True)
    parser.add_argument('--asan', type=Path, required=True)
    parser.add_argument('--baseline-log', type=Path, required=True)
    args = parser.parse_args()
    fixture = 'tools/rhino_oracle/fixtures/native_curve_segment_commands.json'
    observation = 'tools/rhino_oracle/observations/native_curve_segment_commands.json'
    sources = [fixture, observation,
        'tools/usd_brep/qualify_surface_trim_tangencies.py', 'tools/usd_brep/surface_tangent_adapter_probe.cpp',
        'tools/usd_brep/CMakeLists.txt', 'crates/viboceros-smlib/native/bridge.cpp',
        'crates/viboceros-smlib/native/bridge.h', 'crates/viboceros-smlib/native/brep_import.cpp',
        'crates/viboceros-smlib/native/brep_data.h', 'crates/viboceros-smlib/native/brep_export.cpp', 'crates/viboceros-smlib/src/native/working_curve.rs',
        'crates/viboceros-smlib/src/native/curve_surface_intersections.rs',
        'crates/viboceros-smlib/src/native/knot_refinement.rs',
        'crates/viboceros-smlib/src/native/surface_intersections.rs', 'crates/viboceros-smlib/tests/surface_intersections.rs',
        'crates/viboceros-smlib/tests/surface_intersections/trim.rs', 'crates/viboceros-smlib/src/native/curve_intersections.rs',
        'crates/viboceros-geometry/src/intersection.rs', 'crates/viboceros-geometry/src/lib.rs',
        'crates/viboceros-geometry/src/intersection/surface_surface_backend.rs',
        'crates/viboceros-geometry/src/intersection/surface_surface_backend/trim.rs',
        'crates/viboceros-geometry/src/intersection/surface_surface_backend/trim/boundary_span.rs',
        'crates/viboceros-geometry/src/intersection/surface_surface_backend/tests/trim.rs',
        'crates/viboceros-geometry/src/parameter.rs', 'crates/viboceros-geometry/src/brep.rs',
        'crates/viboceros-geometry/src/nurbs2.rs', 'crates/viboceros-geometry/src/nurbs2/evaluate.rs',
        'crates/viboceros-geometry/src/intersection/surface_surface_backend/tests.rs', 'crates/viboceros-geometry/src/surface_pullback/certificate.rs',
        'crates/viboceros-command/src/lib.rs', 'crates/viboceros-command/src/curve_intersections.rs',
        'crates/viboceros-command/src/curve_intersections/tests.rs',
    ]
    baseline_log = args.baseline_log.read_text()
    if 'left: 2' not in baseline_log or 'right: 1' not in baseline_log or 'absolute=0.000001 x=0.45 side=-1' not in baseline_log:
        raise ValueError('missing baseline tangency failure')
    revision = '2b0d305f91089141fed271245ec0b83fa0b159a1'
    baseline_sources = ['crates/viboceros-geometry/src/intersection/surface_surface_backend/trim.rs',
                        'crates/viboceros-geometry/src/intersection/surface_surface_backend.rs',
                        'crates/viboceros-smlib/src/native/surface_intersections.rs',
                        'crates/viboceros-smlib/tests/surface_intersections/trim.rs']
    baseline_hashes = {revision+':'+p:hashlib.sha256(subprocess.check_output(['git','cat-file','blob',revision+':'+p],cwd=ROOT)).hexdigest() for p in baseline_sources}
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
        scope='24 actual C ABI cases per normal/ASan job: fitted general-surface UV traces against tangent rational circles, '
              'on both source witnesses with independent source charts, three planes and both normal offsets. '
              'Zero or multiple native contacts are diagnostic, not evidence of completeness. Original UV and model-space residuals, '
              'input purity and owned snapshot lifetimes checked. Rust separately proves complete correspondence of candidate '
              'near-boundary spans with original trim images, including directed seam pieces; retained adjacent ranges '
              'remain restrictions of the same original XYZ/UV curves. Baseline false fragmentation retained. '
              'Analytic tangency and above-tolerance crossing/clearance regressions at absolute 1e-6 and 1e-9. '
              'All 121 saved intersection/cutting command records replayed at absolute 1e-8, relative 1e-12; '
              'same-request portable/native values retained separately. No fresh Rhino observation, general topology/completeness or speed claim.',
        sanitizer_scope='Complete SMLib, SM_API and C bridge; process-global context leak accounting alone disabled.',
        source_sha256=source_hashes, baseline_production_git_sha256=baseline_hashes,
        baseline=dict(revision=revision, analytic_equation='x^2 + y^3 + x*y = 1', x=0.45,
                      normal_offset=-0.06, radius=0.06, model_tolerance=1e-6, actual_curves=2,
                      expected_connected_curves=1, failure_log=baseline_log),
        jobs=jobs, request=request, native_response=native, portable_response=portable,
        rhino_comparison=rhino, producer_comparison=producer)
    args.output.write_text(json.dumps(record, indent=2, allow_nan=False)+'\n')
    print('Surface trim tangency qualification:', passed, 'SDK matches:', rhino['matched'], '/', rhino['cases'], 'native errors:', rhino['native_failed'])
    if not passed:
        print('Failures:', [c['id'] for c in rhino['operations'] if not c['passed']])
        raise ValueError('qualification failed; all outcomes retained')


if __name__ == '__main__':
    main()
