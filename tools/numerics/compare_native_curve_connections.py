"""Replay retained Connect and pair-filleting observations through usd-brep."""
import argparse
import hashlib
import json
from pathlib import Path

from tools.rhino_oracle import OracleClient, compare_responses, load_request

ROOT = Path(__file__).resolve().parents[2]
FIXTURES = ("connect_nurbs_line", "curve_fillet_pair",
            "curve_fillet_pair_parts", "curve_fillet_nonmeeting")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("--timeout", type=float, default=1800.)
    args = parser.parse_args()
    client = OracleClient(native_smlib=True)
    sources = ["crates/viboceros-geometry/src/curve_connect_pair.rs",
               "crates/viboceros-geometry/src/curve_fillet_pair.rs",
               "crates/viboceros-geometry/src/curve_chamfer_pair.rs",
               "crates/viboceros-geometry/src/nurbs/boundary_extension.rs",
               "crates/viboceros-smlib/src/native/boundary_extension.rs",
               "crates/viboceros-smlib/src/native/curve_connections.rs",
               "crates/viboceros-smlib/src/native/curve_extension.rs",
               "crates/viboceros-smlib/native/knot_refinement.cpp",
               "crates/viboceros-smlib/tests/curve_connections.rs",
               "crates/viboceros-geometry/src/curve_connect_pair/backend_tests.rs",
               "crates/viboceros-command/src/curve_connections.rs",
               "crates/viboceros-command/src/connect.rs",
               "crates/viboceros-command/src/fillet.rs",
               "crates/viboceros-command/src/chamfer.rs",
               "crates/viboceros-oracle/src/curve_connections.rs",
               "crates/viboceros-oracle/src/connect_command.rs",
               "crates/viboceros-oracle/src/lib.rs",
               "tools/rhino_oracle/client.py",
               "tools/numerics/compare_native_curve_connections.py"]
    records = []
    for fixture in FIXTURES:
        request_path = Path("tools/rhino_oracle/fixtures") / f"{fixture}.json"
        reference_path = Path("tools/rhino_oracle/observations") / f"{fixture}.json"
        sources.extend([str(request_path), str(reference_path)])
        request = load_request(ROOT/request_path)
        reference = load_request(ROOT/reference_path)
        native = client.run_viboceros(request, timeout=args.timeout)
        if [r["id"] for r in native["results"]] != [r["id"] for r in reference["results"]]:
            raise ValueError(f"{fixture}: result identifiers differ")
        checks = []
        for a, b in zip(native["results"], reference["results"]):
            # Keep the existing replay ceilings: the NURBS fillet solver uses
            # length sampling, while Connect checks exact ends and controls.
            epsilon = 1e-7 if fixture != "connect_nurbs_line" and "nurbs" in a["id"] else 1e-10
            actual = dict(native, results=[a])
            expected = dict(reference, results=[b])
            checks.append(compare_responses(actual, expected,
                                            absolute_epsilon=epsilon,
                                            relative_epsilon=0.).as_dict())
        record = dict(fixture=fixture, reference=str(reference_path),
                      rhino_engine_version=reference["engine_version"],
                      passed=all(check["passed"] for check in checks),
                      max_absolute_error=max(check["max_absolute_error"] for check in checks),
                      checks=checks, native_response=native)
        records.append(record)
        print(fixture, record["passed"], record["max_absolute_error"], flush=True)
    result = dict(native_smlib=True,
                  upstream_commit="c9c5979f04cea8c38213004d4f04df0dcfb40d05",
                  scope="Saved scripted Connect command and public pair-filleting SDK outputs. "
                        "No fresh Rhino capture, interactive viewport capture, Chamfer SDK "
                        "qualification, rational smooth-Connect parity or kernel-speed claim.",
                  source_sha256={path: hashlib.sha256((ROOT/path).read_bytes()).hexdigest() for path in sources},
                  cases=sum(len(record["checks"]) for record in records),
                  passed=all(record["passed"] for record in records), records=records)
    args.output.write_text(json.dumps(result, indent=2, allow_nan=False)+"\n")
    if not result["passed"]:
        raise ValueError("native connection replay differs; retained all outcomes")


if __name__ == "__main__":
    main()
