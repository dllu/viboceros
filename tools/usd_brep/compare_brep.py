"""Compare exported Rust B-rep fields with the existing public Rhino probe."""
import argparse
import json
import math
from pathlib import Path


def compare(expected, actual, path, errors, stats):
    if isinstance(expected, dict):
        if not isinstance(actual, dict):
            errors.append(dict(path=path, reason="expected object"))
            return
        for key, value in expected.items():
            if key not in actual:
                errors.append(dict(path=path + "/" + key, reason="missing field"))
            else:
                compare(value, actual[key], path + "/" + key, errors, stats)
    elif isinstance(expected, list):
        if not isinstance(actual, list) or len(expected) != len(actual):
            errors.append(dict(path=path, reason="array length"))
            return
        for index, (left, right) in enumerate(zip(expected, actual)):
            compare(left, right, path + "/" + str(index), errors, stats)
    elif isinstance(expected, float):
        if type(actual) not in (int, float) or not math.isfinite(actual):
            errors.append(dict(path=path, reason="nonfinite or nonnumeric"))
            return
        error = abs(expected - actual)
        stats["numeric_fields"] += 1
        stats["maximum_absolute_error"] = max(stats["maximum_absolute_error"], error)
        if error > 1e-12 + 1e-12 * max(abs(expected), abs(actual)):
            errors.append(dict(path=path, reason="numeric tolerance", error=error))
    elif type(expected) is not type(actual) or expected != actual:
        errors.append(dict(path=path, reason="exact value"))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("local", type=Path)
    parser.add_argument("native", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    local = json.loads(args.local.read_text())
    native = json.loads(args.native.read_text())
    actual = {row["id"]: row["value"] for row in native["results"]}
    expected = {row["id"]: row["value"] for row in local["results"]}
    if (len(actual) != len(native["results"]) or len(expected) != len(local["results"])
            or actual.keys() != expected.keys()):
        raise ValueError("reference IDs must be unique and match")
    cases = []
    for name, value in expected.items():
        errors = []
        stats = dict(maximum_absolute_error=0., numeric_fields=0)
        compare(value, actual[name], name, errors, stats)
        cases.append(dict(id=name, qualified=not errors, errors=errors, **stats))
    output = dict(
        native_engine=native["engine_version"], cases=cases,
        absolute_epsilon=1e-12, relative_epsilon=1e-12,
        fields="Exact topology and all expected definitions, tolerances, edge/face and lifted trim samples. Native additional mesh fields are retained separately.",
    )
    args.output.write_text(json.dumps(output, indent=2) + "\n")
    qualified = sum(case["qualified"] for case in cases)
    print("Qualified %d/%d; maximum numeric error %.3g" % (
        qualified, len(cases), max(case["maximum_absolute_error"] for case in cases)))
    return 0 if qualified == len(cases) else 1


if __name__ == "__main__":
    raise SystemExit(main())
