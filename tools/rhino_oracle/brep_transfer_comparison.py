"""Compare native B-rep audit responses, retaining raw trim-start differences."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path

from .join_source_uv_comparison import differences


def without_trims(value):
    return {
        **value,
        "objects": [
            {**o, "brep": {k: v for k, v in o["brep"].items() if k != "trim_curves"}}
            for o in value["objects"]
        ],
    }


def cyclic_trims_equal(a, b):
    for oa, ob in zip(a["objects"], b["objects"]):
        fa, fb = oa["brep"]["trim_curves"], ob["brep"]["trim_curves"]
        if len(fa) != len(fb):
            return False
        for la, lb in zip(fa, fb):
            if len(la) != len(lb):
                return False
            for ca, cb in zip(la, lb):
                if len(ca) != len(cb) or not ca:
                    return False
                if not any(not list(differences(cb, ca[k:] + ca[:k])) for k in range(len(ca))):
                    return False
    return len(a["objects"]) == len(b["objects"])


def comparison(before, after):
    if before["protocol_version"] != after["protocol_version"]:
        raise ValueError("audit protocols differ")
    aa, bb = before["outcomes"], after["outcomes"]
    if len(aa) != len(bb):
        raise ValueError("audit counts differ")
    results = []
    ids = set()
    for a, b in zip(aa, bb):
        if a["status"] != "success" or b["status"] != "success":
            raise ValueError("transfer comparison requires successful audit outcomes")
        a, b = a["result"], b["result"]
        if a["id"] != b["id"] or a["id"] in ids:
            raise ValueError("audit identities or order differ")
        ids.add(a["id"])
        av, bv = a["value"], b["value"]
        delta = list(differences(bv, av))
        outside_a, outside_b = without_trims(av), without_trims(bv)
        outside_equal = not list(differences(outside_b, outside_a))
        results.append(dict(
            id=a["id"], raw_value_match=not delta, raw_difference_count=len(delta),
            outside_trim_values_exact=outside_a == outside_b,
            outside_trim_values_match=outside_equal,
            cyclic_trim_start_only=bool(delta and outside_equal and cyclic_trims_equal(av, bv)),
            first_differences=[dict(path=p, actual=x, baseline=y) for p, x, y in delta[:8]],
        ))
    return dict(
        absolute_epsilon=1e-10, relative_epsilon=1e-12, cases=len(results),
        raw_value_matches=sum(r["raw_value_match"] for r in results),
        outside_trim_values_exact=sum(r["outside_trim_values_exact"] for r in results),
        outside_trim_values_match=sum(r["outside_trim_values_match"] for r in results),
        cyclic_trim_start_only=sum(r["cyclic_trim_start_only"] for r in results),
        policy="Primary comparisons retain raw order and parameterization. Secondary cyclic "
               "comparison only changes each trim list's starting index, using the same epsilon. "
               "No curve fitting, domain normalization or geometry fields are discarded.",
        results=results,
    )


def read(path):
    data = path.read_bytes()
    return gzip.decompress(data) if path.suffix == ".gz" else data


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("before", "after", "output"):
        parser.add_argument(name, type=Path)
    args = parser.parse_args()
    before, after = read(args.before), read(args.after)
    report = comparison(json.loads(before), json.loads(after))
    report["full_response_byte_equal"] = before == after
    report["sha256"] = {str(p): hashlib.sha256(p.read_bytes()).hexdigest()
                        for p in (args.before, args.after)}
    args.output.write_text(json.dumps(report, indent=2, allow_nan=False) + "\n")
    print(json.dumps({k: report[k] for k in ("cases", "raw_value_matches", "cyclic_trim_start_only")}))
    if report["raw_value_matches"] + report["cyclic_trim_start_only"] != report["cases"]:
        raise SystemExit("B-rep fields differ beyond a cyclic trim-list start")


if __name__ == "__main__":
    main()
