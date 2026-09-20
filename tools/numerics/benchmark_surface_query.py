#!/usr/bin/env python3
"""Compare two release oracle executables; no timing-based pass/fail threshold.

Ordinary closest-point operations time the search itself. Sweep operations do
not, so the three signed-weight workloads use whole-process wall time instead.
"""

import argparse
import copy
import hashlib
import json
from pathlib import Path
import platform
import statistics
import struct
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[2]
FIXTURES = ROOT / "tools/rhino_oracle/fixtures"
SIGNED_IDS = {"three-rational-retained-basis", "three-rational-retained-global-basis"}
OFF_SURFACE = [[0.8, 0.9, 2.3], [1.5, -0.25, 4.1], [-0.25, 0.2, 0.7]]


def fingerprint(value):
    """Compare all returned values including binary64 signed-zero bits."""
    if isinstance(value, bool) or value is None or isinstance(value, str):
        return value
    if isinstance(value, (float, int)):
        return ("binary64", struct.pack(">d", value).hex())
    if isinstance(value, list):
        return [fingerprint(v) for v in value]
    return {k: fingerprint(v) for k, v in value.items()}


def read_fixture(name):
    return json.loads((FIXTURES / name).read_text())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("before", type=Path)
    parser.add_argument("after", type=Path)
    parser.add_argument("--cpu", type=int)
    parser.add_argument("--rounds", type=int, default=3)
    parser.add_argument("--baseline-commit", default="unspecified")
    parser.add_argument(
        "--artifacts",
        type=Path,
        help="retain requests and raw responses for value-difference audits",
    )
    args = parser.parse_args()
    if args.rounds < 1:
        parser.error("--rounds must be positive")
    executables = {"before": args.before.resolve(), "after": args.after.resolve()}
    if args.artifacts is not None:
        args.artifacts.mkdir(parents=True, exist_ok=True)
    report = {
        "captured_utc": time.strftime("%Y-%m-%d %H:%M:%S UTC", time.gmtime()),
        "architecture": platform.machine(),
        "baseline_commit": args.baseline_commit,
        "executable_sha256": {
            k: hashlib.sha256(p.read_bytes()).hexdigest()
            for k, p in executables.items()
        },
        "cpu_affinity": args.cpu,
        "ordinary": [],
        "signed": [],
    }
    with tempfile.TemporaryDirectory(prefix="viboceros-query-benchmark-") as directory:
        directory = Path(directory)

        def run(mode, request, label):
            source, output = directory / "request.json", directory / "response.json"
            source.write_text(json.dumps(request))
            output.unlink(missing_ok=True)
            command = [str(executables[mode]), str(source), str(output)]
            if args.cpu is not None:
                command = ["taskset", "-c", str(args.cpu), *command]
            start = time.perf_counter_ns()
            subprocess.run(command, check=True, capture_output=True)
            elapsed = time.perf_counter_ns() - start
            response = json.loads(output.read_text())
            if response.get("error") or len(response["results"]) != len(
                request["operations"]
            ):
                raise RuntimeError(response)
            if args.artifacts is not None:
                (args.artifacts / f"{label}-request.json").write_text(source.read_text())
                (args.artifacts / f"{label}-{mode}.json").write_text(output.read_text())
            return elapsed, response["results"]

        ordinary = read_fixture("surface-closest-point.json")
        ordinary["operations"] += read_fixture("surface-closest-curvature.json")["operations"]
        ordinary["iterations"] = 200
        original = ordinary["operations"]
        ordinary["operations"] = []
        for round_index in range(5):
            for op in original:
                op = copy.deepcopy(op)
                op["id"] += f"-round-{round_index + 1}"
                ordinary["operations"].append(op)
        results = {mode: run(mode, ordinary, "ordinary")[1] for mode in executables}
        report["ordinary_values_bit_identical"] = fingerprint(
            [r["value"] for r in results["before"]]
        ) == fingerprint([r["value"] for r in results["after"]])
        for i, op in enumerate(original):
            record = {
                "id": op["id"],
                "queries_per_batch": len(op["points"]),
                "iterations": 200,
            }
            for mode in executables:
                times = [r["elapsed_ns"] for r in results[mode][i::len(original)]]
                record[f"{mode}_elapsed_ns"] = times
                record[f"{mode}_ns_per_query"] = (
                    statistics.median(times) / 200 / len(op["points"])
                )
            record["speedup"] = (
                record["before_ns_per_query"] / record["after_ns_per_query"]
            )
            report["ordinary"].append(record)

        for workload in ["endpoint", "off_surface", "full_fixture_queries"]:
            request = read_fixture("sweep1_weights.json")
            request["operations"] = [
                op for op in request["operations"] if op["id"] in SIGNED_IDS
            ]
            for op in request["operations"]:
                op["inspect_definition"] = True
                if workload == "endpoint":
                    op["queries"] = op["queries"][:1]
                elif workload == "off_surface":
                    op["queries"] = OFF_SURFACE
            record = {
                "workload": workload,
                "queries_per_operation": [
                    len(op["queries"]) for op in request["operations"]
                ],
                "wall_ns": {mode: [] for mode in executables},
            }
            expected = None
            identical = True
            for round_index in range(args.rounds):
                modes = ["before", "after"]
                if round_index % 2:
                    modes.reverse()
                for mode in modes:
                    elapsed, results = run(mode, request, f"{workload}-{round_index + 1}")
                    record["wall_ns"][mode].append(elapsed)
                    values = fingerprint([r["value"] for r in results])
                    if expected is None:
                        expected = values
                    identical &= values == expected
            record["values_bit_identical_across_all_runs"] = identical
            record["median_wall_ns"] = {
                mode: statistics.median(times)
                for mode, times in record["wall_ns"].items()
            }
            record["speedup"] = (
                record["median_wall_ns"]["before"] / record["median_wall_ns"]["after"]
            )
            report["signed"].append(record)
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
