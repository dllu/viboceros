"""Retain isolated direct-producer results before and after the allocation repair."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]


def capture(executable):
    executable = executable.resolve()
    environment = dict(os.environ)
    environment["LD_LIBRARY_PATH"] = str(executable.parent) + ":" + environment.get("LD_LIBRARY_PATH", "")
    environment["ASAN_OPTIONS"] = "detect_leaks=0:halt_on_error=1"
    results = []
    for index in range(8):
        result = subprocess.run([str(executable), str(index)], capture_output=True,
                                text=True, timeout=30, env=environment)
        record = dict(index=index, returncode=result.returncode,
                      stdout=result.stdout, stderr=result.stderr)
        try:
            record["output"] = json.loads(result.stdout)
        except json.JSONDecodeError:
            record["output"] = None
        results.append(record)
    return results


def successful(record):
    output = record["output"]
    return (record["returncode"] == 0 and not record["stderr"]
            and output is not None and output.get("passed") is True
            and len(output["cases"]) == 1)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("before", type=Path)
    parser.add_argument("after", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--asan-before", type=Path)
    parser.add_argument("--asan-after", type=Path)
    args = parser.parse_args()
    records = dict(before=capture(args.before), after=capture(args.after))
    if args.asan_before:
        records["asan_before"] = capture(args.asan_before)
    if args.asan_after:
        records["asan_after"] = capture(args.asan_after)
    sources = ["third_party/usd-brep/source/SMLib/src/NL_CrvAdv.cpp",
               "third_party/patches/usd-brep-extension-break-allocation.patch",
               "tools/usd_brep/CMakeLists.txt", "tools/usd_brep/extension_probe.cpp",
               "tools/usd_brep/compare_curve_extension_producer.py"]
    result = dict(upstream_commit="c9c5979f04cea8c38213004d4f04df0dcfb40d05",
                  scope="Degree-two linear pieces, with or without a full-order interior break; "
                        "both ends and parameter/distance extension; analytic position and domain checks.",
                  maximum_point_epsilon=1e-10, maximum_domain_epsilon=1e-10,
                  asan_options="detect_leaks=0:halt_on_error=1",
                  source_sha256={path: hashlib.sha256((ROOT/path).read_bytes()).hexdigest() for path in sources},
                  records=records)
    # Preserve all failures, including signals and sanitizer diagnostics.
    args.output.write_text(json.dumps(result, indent=2, allow_nan=False)+"\n")
    for name, checks in records.items():
        if name in ("after", "asan_after"):
            if not all(successful(check) for check in checks):
                raise ValueError(f"{name}: patched producer failed")
        elif (not all(successful(check) for check in checks[:4])
              or all(successful(check) for check in checks[4:])):
            raise ValueError(f"{name}: expected baseline contrast was not reproduced")
        print(name, sum(successful(check) for check in checks), "of 8 passed", flush=True)


if __name__ == "__main__":
    main()
