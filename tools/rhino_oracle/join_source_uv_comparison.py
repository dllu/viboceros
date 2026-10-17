"""Report raw Join archive differences before and after source UV restoration.

Use the Rust Join replay's numeric epsilon. Preserve table order, face senses,
parameterizations and stored tolerances; these counts are not spatial distances.
"""
import argparse
from collections import Counter
import hashlib
import json
import math
from pathlib import Path


def differences(actual, expected, path=""):
    numeric = lambda value: type(value) in (int, float)
    if numeric(actual) and numeric(expected):
        epsilon = max(1e-10, 1e-12 * max(abs(actual), abs(expected)))
        if not (math.isfinite(actual) and math.isfinite(expected)
                and abs(actual - expected) <= epsilon):
            yield path, actual, expected
    elif type(actual) is not type(expected):
        yield path, actual, expected
    elif isinstance(actual, list):
        if len(actual) != len(expected):
            yield path + "/@length", len(actual), len(expected)
        for i, (a, b) in enumerate(zip(actual, expected)):
            yield from differences(a, b, f"{path}/{i}")
    elif isinstance(actual, dict):
        if actual.keys() != expected.keys():
            yield path + "/@keys", sorted(actual), sorted(expected)
        for key in sorted(actual.keys() & expected.keys()):
            yield from differences(actual[key], expected[key], f"{path}/{key}")
    elif actual != expected:
        yield path, actual, expected


def summary(actual, expected):
    delta = list(differences(actual, expected))
    fields = Counter()
    for path, _, _ in delta:
        parts = path.strip("/").split("/")
        if len(parts) > 3 and parts[0] == "objects":
            field = "brep." + parts[3] if parts[2] == "brep" else "object." + parts[2]
        else:
            field = parts[0]
        fields[field] += 1
    return {
        "matched": not delta,
        "difference_count": len(delta),
        "differences_by_field": dict(sorted(fields.items())),
        "first_differences": [dict(path=p, actual=a, expected=b) for p, a, b in delta[:8]],
    }


def comparison(before, after, rhino):
    if not before["protocol_version"] == after["protocol_version"] == rhino["protocol_version"]:
        raise ValueError("Join response protocols differ")
    ids = [r["id"] for r in rhino["results"]]
    if len(ids) != len(set(ids)):
        raise ValueError("duplicate Rhino Join case")
    for response in (before, after):
        if [r["id"] for r in response["results"]] != ids:
            raise ValueError("Join response identities or order differ")
    results = [
        {"id": a["id"], "before": summary(a["value"], r["value"]),
         "after": summary(b["value"], r["value"])}
        for a, b, r in zip(before["results"], after["results"], rhino["results"])
    ]
    return {
        "absolute_epsilon": 1e-10,
        "relative_epsilon": 1e-12,
        "policy": "Raw table order, parameterizations, senses and tolerances are compared. "
                  "Array/key shape changes count once, then common entries are compared. "
                  "Difference counts do not measure physical boundary deviation.",
        "cases": len(results),
        "before_matched_cases": sum(r["before"]["matched"] for r in results),
        "after_matched_cases": sum(r["after"]["matched"] for r in results),
        "results": results,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("before", "after", "rhino", "output"):
        parser.add_argument(name, type=Path)
    args = parser.parse_args()
    paths = (args.before, args.after, args.rhino)
    report = comparison(*(json.loads(p.read_text()) for p in paths))
    report["sha256"] = {str(p): hashlib.sha256(p.read_bytes()).hexdigest() for p in paths}
    report["sha256"][str(Path(__file__).resolve().relative_to(Path.cwd()))] = hashlib.sha256(
        Path(__file__).read_bytes()
    ).hexdigest()
    args.output.write_text(json.dumps(report, indent=2, allow_nan=False) + "\n")
    print(json.dumps({k: report[k] for k in ("cases", "before_matched_cases", "after_matched_cases")}))


if __name__ == "__main__":
    main()
