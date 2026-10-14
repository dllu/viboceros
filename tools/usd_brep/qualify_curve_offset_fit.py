"""Retain analytic native offset probes, complete ASan and saved Rhino replay."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

from tools.rhino_oracle import OracleClient, load_request
from tools.rhino_oracle.client import _compare_value

ROOT = Path(__file__).resolve().parents[2]


def probe(path, sanitize=False, hermite=False):
    env = dict(os.environ)
    if sanitize:
        env["ASAN_OPTIONS"] = "detect_leaks=0"
    result = subprocess.run([str(path.resolve())], capture_output=True, text=True,
                            env=env, timeout=120, check=False)
    response = json.loads(result.stdout) if result.returncode == 0 else result.stdout
    passed = result.returncode == 0 and isinstance(response, dict)
    if passed and hermite:
        passed = len(response["cases"]) == 6 and all(
            c["status"] == 1000 and 0 <= c["independent_point_error"] <= c["tolerance"]
            for c in response["cases"])
    elif passed:
        passed = response.get("passed") is True and len(response["cases"]) == 48 and all(
            c["passed"] and 0 <= c["point_error"] <= c["tolerance"]
            for c in response["cases"])
    return dict(return_code=result.returncode, stderr=result.stderr, response=response,
                passed=passed, binary_sha256=hashlib.sha256(path.read_bytes()).hexdigest())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("--probe", type=Path, required=True)
    parser.add_argument("--asan", type=Path, required=True)
    parser.add_argument("--hermite", type=Path, required=True)
    args = parser.parse_args()
    jobs = dict(native=probe(args.probe), address_sanitizer=probe(args.asan, True),
                generic_hermite=probe(args.hermite, hermite=True))
    fixture_path = "tools/rhino_oracle/fixtures/curve_offset_corners.json"
    observation_path = "tools/rhino_oracle/observations/curve_offset_corners.json"
    request = load_request(ROOT / fixture_path)
    native = OracleClient(native_smlib=True).run_viboceros(request, timeout=1800)
    rhino = json.loads((ROOT / observation_path).read_text())
    checks = []
    if [r["id"] for r in native["results"]] != [r["id"] for r in rhino["results"]]:
        raise ValueError("saved Rhino/native identifiers differ")
    for actual, expected in zip(native["results"], rhino["results"]):
        differences = []
        error = _compare_value(actual["id"], actual["value"], expected["value"],
                               1e-7, 0., differences)
        checks.append(dict(id=actual["id"], passed=not differences,
                           max_absolute_error=error, differences=differences))
    sources = [fixture_path, observation_path,
               "tools/usd_brep/qualify_curve_offset_fit.py", "tools/usd_brep/CMakeLists.txt",
               "tools/usd_brep/offset_adapter_probe.cpp", "tools/usd_brep/offset_hermite_probe.cpp",
               "crates/viboceros-smlib/native/bridge.cpp", "crates/viboceros-smlib/native/bridge.h",
               "crates/viboceros-smlib/src/native/working_curve.rs",
               "crates/viboceros-smlib/src/native/curve_offset_fit.rs",
               "crates/viboceros-smlib/src/native/curve_offset.rs",
               "crates/viboceros-smlib/tests/curve_offset_fit.rs",
               "crates/viboceros-geometry/src/curve_offset.rs",
               "crates/viboceros-geometry/src/curve_offset/nurbs.rs",
               "crates/viboceros-geometry/src/curve_offset/nurbs/closed_offset.rs",
               "crates/viboceros-oracle/src/curve_offset.rs",
               "third_party/usd-brep/source/SMLib/src/SmCurve.cpp",
               "third_party/usd-brep/source/SMLib/src/SmOffsetCurve.cpp"]
    record = dict(upstream_commit="c9c5979f04cea8c38213004d4f04df0dcfb40d05",
                  scope="48 actual C ABI analytic parabola/rational-circle cases per job: "
                        "two charts, three planes, both sides and two tolerances. "
                        "Exported controls independently evaluated at 1025 stations. "
                        "Six generic Hermite cases at 4097 stations. Four existing Rhino "
                        "corner records replayed at their unchanged 1e-7 epsilon; no fresh capture.",
                  sanitizer_scope="Complete SMLib, SM_API and C bridge; only process-global context leak accounting disabled.",
                  passed=all(j["passed"] for j in jobs.values()) and all(c["passed"] for c in checks),
                  source_sha256={p: hashlib.sha256((ROOT / p).read_bytes()).hexdigest() for p in sources},
                  jobs=jobs, rhino_checks=checks, native_response=native)
    args.output.write_text(json.dumps(record, indent=2, allow_nan=False) + "\n")
    print("Native offset, ASan and saved Rhino:", record["passed"])
    if not record["passed"]:
        raise ValueError("offset qualification failed; retained all outcomes")


if __name__ == "__main__":
    main()
