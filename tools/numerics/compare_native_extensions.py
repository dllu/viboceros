"""Compare native extension outputs with Rhino or the portable producer."""
import argparse
import copy
import hashlib
import json
from pathlib import Path

from tools.rhino_oracle import OracleClient, compare_responses, load_request
# Reuse the protocol's value comparison without relabeling a portable response
# as Rhino merely to pass the engine-specific response validator.
from tools.rhino_oracle.client import _compare_value

ROOT = Path(__file__).resolve().parents[2]
FIXTURES = ("curve_extend", "curve_extend_length", "surface_extend",
            "surface_extend_length", "curve_extend_command", "curve_extend_arc_command")
REFERENCE = Path("tools/rhino_oracle/observations/native_extensions.json")


def combined_request():
    operations = []
    for fixture in FIXTURES:
        source = load_request(ROOT/"tools/rhino_oracle/fixtures"/(fixture+".json"))
        if {k: v for k, v in source.items() if k != "operations"} != dict(protocol_version=1, iterations=1):
            raise ValueError("extension fixture header changed; review batch settings")
        for operation in source["operations"]:
            operation = copy.deepcopy(operation)
            operation["id"] = fixture+"/"+operation["id"]
            operations.append(operation)
    return dict(protocol_version=1, iterations=1, operations=operations)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    reference_mode = parser.add_mutually_exclusive_group()
    reference_mode.add_argument("--capture-rhino", action="store_true")
    reference_mode.add_argument("--portable-baseline", action="store_true")
    parser.add_argument("--scheme", default="VibocerosOracleNativeExtensions20261014")
    parser.add_argument("--timeout", type=float, default=1800.)
    args = parser.parse_args()
    request = combined_request()
    client = OracleClient(native_smlib=True, settings_scheme=args.scheme)
    if args.portable_baseline:
        reference = OracleClient(native_smlib=False).run_viboceros(request, timeout=args.timeout)
        print("Captured", len(reference["results"]), "portable outputs", flush=True)
    elif args.capture_rhino:
        reference = client.run_rhino(request, timeout=args.timeout)
        (ROOT/REFERENCE).write_text(json.dumps(reference, indent=2, allow_nan=False)+"\n")
        print("Captured", len(reference["results"]), "fresh Rhino outputs", flush=True)
    else:
        reference = load_request(ROOT/REFERENCE)
    native = client.run_viboceros(request, timeout=args.timeout)
    records = []
    for fixture in FIXTURES:
        def subset(response):
            response = dict(response)
            response["results"] = [r for r in response["results"] if r["id"].startswith(fixture+"/")]
            return response
        # Keep the documented command ceilings. Curve length uses the same
        # shared numerical length solve; exact parameter extrapolation is tighter.
        length_or_command = fixture in ("curve_extend_length", "curve_extend_command", "curve_extend_arc_command")
        if args.portable_baseline:
            local = subset(native)["results"]
            baseline = subset(reference)["results"]
            if [r["id"] for r in local] != [r["id"] for r in baseline]:
                raise ValueError("native and portable result identifiers differ")
            checks = []
            for a, b in zip(local, baseline):
                differences = []
                error = _compare_value(a["id"], a["value"], b["value"], 1e-10, 1e-12, differences)
                checks.append(dict(id=a["id"], passed=not differences,
                                   max_absolute_error=error, differences=differences))
            record = dict(fixture=fixture, absolute_epsilon=1e-10, relative_epsilon=1e-12,
                          passed=all(check["passed"] for check in checks),
                          max_absolute_error=max(check["max_absolute_error"] for check in checks),
                          operations=checks)
        else:
            comparison = compare_responses(subset(native), subset(reference),
                                           absolute_epsilon=2e-8 if length_or_command else 1e-10,
                                           relative_epsilon=1e-11 if length_or_command else 1e-10)
            record = dict(fixture=fixture, **comparison.as_dict())
        records.append(record)
        print(fixture, record["passed"], record["max_absolute_error"], flush=True)
    sources = ["crates/viboceros-geometry/src/nurbs.rs",
               "crates/viboceros-geometry/src/nurbs_surface.rs",
               "crates/viboceros-smlib/native/knot_refinement.cpp",
               "crates/viboceros-smlib/src/native/curve_extension.rs",
               "crates/viboceros-command/src/extension_backend.rs",
               "crates/viboceros-oracle/src/extension_backend.rs",
               "tools/numerics/compare_native_extensions.py", "tools/rhino_oracle/client.py"]
    if not args.portable_baseline:
        sources.append(str(REFERENCE))
    sources += [f"tools/rhino_oracle/fixtures/{fixture}.json" for fixture in FIXTURES]
    result = dict(native_smlib=True, cases=len(request["operations"]),
                  reference="portable producer" if args.portable_baseline else str(REFERENCE),
                  reference_engine=reference["engine"],
                  scope=("Native and portable producers on the same fixtures; this does not qualify Rhino parity."
                         if args.portable_baseline else
                         "Public SDK geometry and scripted Extend command outputs, including untouched Arc/Line "
                         "policies; no interactive viewport capture or kernel-speed claim."),
                  source_sha256={path: hashlib.sha256((ROOT/path).read_bytes()).hexdigest() for path in sources},
                  passed=all(record["passed"] for record in records),
                  records=records, native_response=native)
    if args.portable_baseline:
        result["portable_reference_response"] = reference
    args.output.write_text(json.dumps(result, indent=2, allow_nan=False)+"\n")
    if not result["passed"]:
        raise ValueError("native extension replay differs from its reference; retained all outcomes")


if __name__ == "__main__":
    main()
