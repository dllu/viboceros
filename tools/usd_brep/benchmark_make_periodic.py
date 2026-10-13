#!/usr/bin/env python3
"""Compare two Viboceros MakePeriodic producers and their complete definitions."""
from __future__ import annotations

import argparse
import hashlib
import json
import math
from pathlib import Path
import platform
import statistics
import subprocess
import sys
import tempfile

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
from tools.rhino_oracle.client import _compare_value, _validate_response


def request(iterations: int) -> dict:
    operations = []
    for degree, counts in [(3, [32, 128, 256, 512]), (11, [256, 512])]:
        for unique in counts:
            count = unique + degree
            controls = []
            for i in range(count):
                t = math.tau * (i % (count - 1)) / (count - 1)
                controls.append({"point": [math.cos(t), math.sin(t), 0.1 * math.sin(2 * t)]})
            for nonuniform in [False, True]:
                breaks = [i * i / unique if nonuniform else i for i in range(unique + 1)]
                operations.append({
                    "op": "curve_make_periodic_geometry",
                    "id": f"degree-{degree}-unique-{unique}-{'nonuniform' if nonuniform else 'uniform'}",
                    "curve": {"degree": degree, "control_points": controls,
                              "knots": [0] * degree + breaks + [unique] * degree},
                    "smooth": True,
                })
    return {"protocol_version": 1, "iterations": iterations, "operations": operations}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--native", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--runs", type=int, default=5)
    parser.add_argument("--iterations", type=int, default=5)
    args = parser.parse_args()
    if args.runs < 1 or not 1 <= args.iterations <= 1_000_000:
        parser.error("runs must be positive and iterations must be 1..1,000,000")
    fixture = request(args.iterations)
    timings = {name: {} for name in ["baseline", "native"]}
    definitions = {}
    with tempfile.TemporaryDirectory(prefix="viboceros-periodic-") as directory:
        input_path = Path(directory) / "request.json"
        input_path.write_text(json.dumps(fixture))
        for run in range(args.runs):
            # Alternate order to reduce a consistent thermal/scheduling bias.
            order = ["baseline", "native"] if run % 2 == 0 else ["native", "baseline"]
            for name in order:
                output_path = Path(directory) / f"{name}.json"
                subprocess.run([str(getattr(args, name).resolve()), str(input_path), str(output_path)], check=True)
                response = json.loads(output_path.read_text())
                _validate_response(response, "viboceros")
                assert {r["id"] for r in response["results"]} == {o["id"] for o in fixture["operations"]}
                for result in response["results"]:
                    timings[name].setdefault(result["id"], []).append(result["elapsed_ns"] / args.iterations)
                    definitions.setdefault(name, {})[result["id"]] = result["value"]
    rows = []
    for operation in fixture["operations"]:
        operation_id = operation["id"]
        differences = []
        error = _compare_value(operation_id, definitions["baseline"][operation_id],
                               definitions["native"][operation_id], 1e-9, 1e-12, differences)
        baseline = statistics.median(timings["baseline"][operation_id])
        native = statistics.median(timings["native"][operation_id])
        rows.append({"id": operation_id, "passed": not differences,
                     "max_absolute_error": error, "differences": differences,
                     "baseline_ns_per_iteration": timings["baseline"][operation_id],
                     "native_ns_per_iteration": timings["native"][operation_id],
                     "median_baseline_ns": baseline, "median_native_ns": native,
                     "baseline_to_native_ratio": baseline / native})
    report = {
        "note": "Both executables are Viboceros; these are diagnostic producer timings, not Rhino measurements. Each operation has an untimed warmup; serialization is excluded. Other compilation may affect timing.",
        "platform": platform.platform(), "runs": args.runs, "iterations": args.iterations,
        "binary_sha256": {name: hashlib.sha256(getattr(args, name).read_bytes()).hexdigest()
                          for name in ["baseline", "native"]},
        "absolute_epsilon": 1e-9, "relative_epsilon": 1e-12,
        "passed": all(row["passed"] for row in rows), "operations": rows,
    }
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    for row in rows:
        print(f"{row['id']}: {row['baseline_to_native_ratio']:.2f}x, match={row['passed']}")
    if not report["passed"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
