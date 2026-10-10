"""Qualify exact 3DM partitions of the curved_split cylinder/slab example."""
import argparse
import json
import math
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("native", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    native = json.loads(args.native.read_text())
    rows, ranges = [], []
    for record in native["results"]:
        value = record["value"]
        if not value["topology"]["solid"]:
            raise ValueError("partition is not a native solid")
        vertices = [row["point"] for row in value["vertices"]]
        low, high = min(p[2] for p in vertices), max(p[2] for p in vertices)
        ranges.append([low, high])
        maximum, count = 0., 0
        # Both edge samples and surface-lifted trim samples lie on the actual partition.
        samples = [p for e in value["edges"] for p in e["curve"]["samples"]]
        samples += [p for f in value["faces"] for loop in f["loops"] for t in loop for p in t["lifted"]]
        for x, y, z in samples:
            if not all(math.isfinite(v) for v in (x, y, z)):
                raise ValueError("nonfinite boundary sample")
            radial = abs(math.hypot(x, y)-2.)
            cap = min(abs(z-low), abs(z-high)) if math.hypot(x, y) <= 2.+1e-8 else math.inf
            error = min(radial, cap)
            if z < low-1e-8 or z > high+1e-8 or error >= 1e-8:
                raise ValueError("sample leaves its cylinder partition")
            maximum, count = max(maximum, error), count+1
        rows.append(dict(id=record["id"], native_solid=True, z_range=[low, high],
                         boundary_samples=count, maximum_boundary_error=maximum,
                         faces=len(value["faces"]), edges=len(value["edges"]),
                         display_mesh=value["mesh"]))
    ranges.sort()
    expected = [[0., 4.], [4., 6.], [6., 10.]]
    if len(ranges) != 3 or any(abs(a-b)>1e-8 for pair, reference in zip(ranges, expected) for a,b in zip(pair,reference)):
        raise ValueError("expected three adjacent cylinder ranges")
    args.output.write_text(json.dumps(dict(engine=native["engine_version"], cases=rows,
        geometric_epsilon=1e-8, qualified_ranges=ranges,
        scope="Native solid validity and boundary samples qualify these artifacts; full interactive partition parity remains unproven."), indent=2)+"\n")
    print("Qualified three native pieces and %d boundary samples" % sum(r["boundary_samples"] for r in rows))


if __name__ == "__main__":
    main()
