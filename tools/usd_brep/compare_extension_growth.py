"""Retain kernel growth and real adapter results, including sanitizer outcomes."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]


def capture(executable):
    executable = executable.resolve()
    environment = dict(os.environ, LD_LIBRARY_PATH=str(executable.parent),
                       ASAN_OPTIONS="detect_leaks=0:halt_on_error=1")
    result = subprocess.run([str(executable)], capture_output=True, text=True,
                            timeout=60, env=environment)
    record = dict(returncode=result.returncode, stdout=result.stdout, stderr=result.stderr)
    try:
        record["output"] = json.loads(result.stdout)
    except json.JSONDecodeError:
        record["output"] = None
    return record


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    baseline = parser.add_mutually_exclusive_group(required=True)
    baseline.add_argument("--before", type=Path)
    baseline.add_argument("--before-record", type=Path)
    parser.add_argument("--after", type=Path, required=True)
    parser.add_argument("--adapter", type=Path, required=True)
    parser.add_argument("--asan", type=Path, required=True)
    parser.add_argument("--asan-adapter", type=Path, required=True)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    before = capture(args.before) if args.before else json.loads(args.before_record.read_text())
    if "output" not in before:
        before["output"] = json.loads(before["stdout"])
    records = dict(before=before, after=capture(args.after), adapter=capture(args.adapter),
                   asan=capture(args.asan), asan_adapter=capture(args.asan_adapter))
    sources = ["third_party/usd-brep/source/SMLib/src/NL_CrvTool.cpp",
               "third_party/patches/usd-brep-knot-removal-bound-error.patch",
               "tools/usd_brep/extension_growth_probe.cpp",
               "tools/usd_brep/extension_adapter_probe.cpp", "tools/usd_brep/CMakeLists.txt",
               "tools/usd_brep/compare_extension_growth.py",
               "crates/viboceros-smlib/native/knot_refinement.cpp"]
    output = dict(upstream_commit="c9c5979f04cea8c38213004d4f04df0dcfb40d05",
                  scope="Fixed polynomial end-span growth and real C ABI extrapolation; source grid uses "
                        "absolute error, distant grid uses relative error; no Rhino capture.",
                  source_point_epsilon=1e-9, relative_point_epsilon=1e-12,
                  asan_options="detect_leaks=0:halt_on_error=1", records=records,
                  source_sha256={path: hashlib.sha256((ROOT/path).read_bytes()).hexdigest() for path in sources})
    args.output.write_text(json.dumps(output, indent=2, allow_nan=False)+"\n")
    if before["returncode"] == 0:
        raise ValueError("baseline no longer reproduces the native failure")
    for name, record in records.items():
        if name == "before":
            continue
        if record["returncode"] or record["stderr"] or not record["output"] or not record["output"]["passed"]:
            raise ValueError(f"{name}: producer failed; retained diagnostics")
        print(name, len(record["output"]["cases"]), "cases passed", flush=True)


if __name__ == "__main__":
    main()
