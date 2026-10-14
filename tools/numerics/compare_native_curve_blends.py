"""Replay the retained Rhino blend definitions through the native Python API."""
import argparse
import hashlib
import json
import math
from pathlib import Path

from tools.rhino_oracle import OracleClient, load_request

ROOT = Path(__file__).resolve().parents[2]
FIXTURES = ("blend_lines", "blend_parallel_lines", "blend_mixed_spatial",
            "blend_endpoint_picks", "blend_curved_sources", "blend_nurbs_sources",
            "blend_polycurve_sources", "blend_bulges")


def compare(actual, expected):
    if actual["degree"] != expected["degree"]:
        raise ValueError("blend degree differs")
    controls = actual["control_points"]
    if len(controls) != len(expected["control_points"]):
        raise ValueError("control counts differ")
    control_error = 0.
    for a, b in zip(controls, expected["control_points"]):
        if len(a["point"]) != 3 or len(b["point"]) != 3:
            raise ValueError("control points must have three coordinates")
        if a["weight"] != b["weight"]:
            raise ValueError("polynomial weights differ")
        for x, y in zip(a["point"], b["point"]):
            error = abs(x-y)
            if not math.isfinite(error) or error > 1e-12:
                raise ValueError(f"control coordinate exceeds 1e-12: {error}")
            control_error = max(control_error, error)
    parameter_error = 0.
    for key in ("domain", "knots"):
        if len(actual[key]) != len(expected[key]):
            raise ValueError(f"{key} lengths differ")
        for x, y in zip(actual[key], expected[key]):
            error = abs(x-y)
            if not math.isfinite(error) or error > 5e-8:
                raise ValueError(f"{key} exceeds the retained arc-length ceiling: {error}")
            parameter_error = max(parameter_error, error)
    return dict(maximum_control_coordinate_error=control_error,
                maximum_parameter_error=parameter_error)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("--timeout", type=float, default=1800.)
    args = parser.parse_args()
    client = OracleClient(native_smlib=True)
    sources = ["crates/viboceros-geometry/src/endpoint_interpolation.rs",
               "crates/viboceros-geometry/src/curve_blend_pair.rs",
               "crates/viboceros-smlib/src/native/knot_refinement.rs",
               "crates/viboceros-smlib/src/native/curve_blend.rs",
               "crates/viboceros-oracle/src/blend_curve.rs",
               "tools/rhino_oracle/client.py",
               "tools/numerics/compare_native_curve_blends.py"]
    records = []
    for fixture in FIXTURES:
        request_path = Path("tools/rhino_oracle/fixtures") / f"{fixture}.json"
        reference_path = Path("tools/rhino_oracle/observations") / f"{fixture}.json"
        sources.extend([str(request_path), str(reference_path)])
        request = load_request(ROOT/request_path)
        reference = load_request(ROOT/reference_path)
        native = client.run_viboceros(request, timeout=args.timeout)
        if len(native["results"]) != len(reference["results"]):
            raise ValueError(f"{fixture}: result counts differ")
        checks = []
        for a, b in zip(native["results"], reference["results"]):
            if a["id"] != b["id"]:
                raise ValueError("fixture identifiers differ")
            checks.append(dict(id=a["id"], **compare(a["value"], b["value"])))
        records.append(dict(fixture=fixture, rhino_engine_version=reference["engine_version"],
                            checks=checks, native_response=native))
        print(f"Qualified {fixture}: {len(checks)} native curve blends", flush=True)
    result = dict(native_smlib=True,
                  upstream_commit="c9c5979f04cea8c38213004d4f04df0dcfb40d05",
                  control_coordinate_epsilon=1e-12, arc_length_parameter_epsilon=5e-8,
                  scope="Public native curve-blend outputs compared with retained Rhino SDK definitions; "
                        "this is not a fresh Rhino session or interactive command capture.",
                  source_sha256={path: hashlib.sha256((ROOT/path).read_bytes()).hexdigest() for path in sources},
                  cases=sum(len(r["checks"]) for r in records), records=records)
    args.output.write_text(json.dumps(result, indent=2, allow_nan=False)+"\n")


if __name__ == "__main__":
    main()
