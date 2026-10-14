"""Retain native curve-filleting side repair and unresolved scale failures."""
import argparse
from decimal import Decimal, localcontext
import hashlib
import json
import math
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]


def independent_contacts():
    with localcontext() as context:
        context.prec = 80
        low, high = Decimal(0), Decimal(1)
        for _ in range(270):
            t = (low + high) / 2
            norm = (t*t + (1-t)*(1-t)).sqrt()
            value = 2*t*t - Decimal("0.5")*(1-t)/norm
            if value < Decimal("1.5"):
                low = t
            else:
                high = t
        t = (low + high) / 2
        norm = (t*t + (1-t)*(1-t)).sqrt()
        start = (4*t - 2*t*t, 2*t*t, Decimal(0))
        cx = start[0] + Decimal("0.5")*t/norm
        return float(t), [float(v) for v in start], [float(cx), 2., 0.], float(cx)


def run(path, sanitize=False):
    environment = dict(os.environ)
    if sanitize:
        environment["ASAN_OPTIONS"] = "detect_leaks=0"
    result = subprocess.run([str(path.resolve())], capture_output=True, text=True,
                            env=environment, timeout=120, check=False)
    record = dict(executable=str(path), binary_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
                  return_code=result.returncode, stderr=result.stderr)
    record["response"] = json.loads(result.stdout) if result.returncode == 0 else result.stdout
    return record


def qualify(record):
    t, start, end, cx = independent_contacts()
    angle = math.atan2(start[1]-1.5, start[0]-cx)
    mid = [cx+0.5*math.cos((angle+math.pi/2)/2),
           1.5+0.5*math.sin((angle+math.pi/2)/2), 0.]
    checks = []
    if record["return_code"] != 0:
        return checks
    cases = record["response"]["cases"]
    if len(cases) != 9:
        raise ValueError("expected all nine producer jobs")
    for case in cases:
        check = dict(scale=case["scale"], chart=case["chart"], status=case["status"], qualified=False)
        if case["status"] == 1000:
            chart_start = [0., 10., -8.][case["chart"]]
            first, second = case["retained_domains"]
            error = 0.
            for sample, expected in zip(case["fillet_samples"], [start, mid, end]):
                if sample["status"] != 1000:
                    raise ValueError("native fillet sample evaluation failed")
                error = max(error, *(abs(a/case["scale"]-b) for a, b in zip(sample["point"], expected)))
            outer = [case["first_start"], case["second_end"]]
            for sample, expected in zip(outer, [[0.,0.,0.], [4.,2.,0.]]):
                if sample["status"] != 1000:
                    raise ValueError("native retained endpoint evaluation failed")
                error = max(error, *(abs(a/case["scale"]-b) for a, b in zip(sample["point"], expected)))
            parameter_error = max(abs(first[0]), abs(first[1]-t),
                                  abs(second[0]-(chart_start+(cx-2)/2)),
                                  abs(second[1]-(chart_start+1)))
            check.update(maximum_normalized_coordinate_error=error,
                         maximum_parameter_error=parameter_error,
                         qualified=error < 1e-10 and parameter_error < 1e-10)
        checks.append(check)
    return checks


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--corrected", type=Path, required=True)
    parser.add_argument("--asan", type=Path, required=True)
    args = parser.parse_args()
    jobs = {name: run(path, name == "asan") for name, path in
            [("baseline", args.baseline), ("corrected", args.corrected), ("asan", args.asan)]}
    for job in jobs.values():
        job["checks"] = qualify(job)
    corrected = jobs["corrected"]["checks"]
    baseline = jobs["baseline"]["checks"]
    fixed = (sum(c["qualified"] for c in corrected) == 6
             and any(c["scale"] == 1 and c["chart"] == 1 and c["status"] != 1000 for c in baseline))
    safety = jobs["asan"]["return_code"] == 0 and sum(c["qualified"] for c in jobs["asan"]["checks"]) == 6
    sources = ["tools/usd_brep/curve_fillet_probe.cpp", "tools/usd_brep/compare_curve_fillet_producer.py",
               "tools/usd_brep/CMakeLists.txt", "crates/viboceros-smlib/native/CMakeLists.txt",
               "third_party/patches/usd-brep-curve-fillet-sides.patch",
               "third_party/usd-brep/source/SMLib/src/SmCurveFilletExecutive.cpp"]
    record = dict(upstream_commit="c9c5979f04cea8c38213004d4f04df0dcfb40d05",
                  scope="Direct native executive; independent 80-digit quadratic contacts. "
                        "All jobs retained, including small-scale failures; no Rhino or speed qualification.",
                  side_fix_verified=fixed, address_sanitizer_passed=safety,
                  sanitizer_scope="Complete SMLib and SM_API; only process-global context leak accounting disabled.",
                  qualified_corrected_cases=sum(c["qualified"] for c in corrected),
                  unqualified_corrected_cases=[c for c in corrected if not c["qualified"]],
                  source_sha256={path: hashlib.sha256((ROOT/path).read_bytes()).hexdigest() for path in sources},
                  jobs=jobs)
    args.output.write_text(json.dumps(record, indent=2, allow_nan=False)+"\n")
    print("Side repair:", fixed, "ASan:", safety, "Qualified:", record["qualified_corrected_cases"], "of 9")
    if not fixed or not safety:
        raise ValueError("side repair or sanitizer qualification failed; retained all jobs")


if __name__ == "__main__":
    main()
