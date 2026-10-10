"""Qualify analytic boundary samples of the curved_sets command artifacts."""
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
    rows = []
    for record in native["results"]:
        name, value = record["id"], record["value"]
        if name not in ("union", "common_intersection", "two_set_intersection"):
            raise ValueError("unknown fixed command artifact")
        maximum, count = 0., 0
        for edge in value["edges"]:
            for x, y, z in edge["curve"]["samples"]:
                if not all(math.isfinite(v) for v in (x, y, z)):
                    raise ValueError("nonfinite edge sample")
                sphere = abs(math.sqrt(x*x + y*y + (z-10.)**2) - 2.)
                box = min(abs(abs(x)-5.), abs(abs(y)-5.), abs(z), abs(z-10.))
                in_box = -5.-1e-8 <= x <= 5.+1e-8 and -5.-1e-8 <= y <= 5.+1e-8 and -1e-8 <= z <= 10.+1e-8
                error = min(sphere, box if in_box else math.inf) if name == "union" else min(sphere, abs(z-10.))
                if error >= 1e-8:
                    raise ValueError("edge sample leaves analytic boundary")
                maximum, count = max(maximum, error), count + 1
        surface_maximum, surface_count = 0., 0
        for face in value["faces"]:
            points = face["samples"]
            planar = any(max(p[a] for p in points) - min(p[a] for p in points) < 1e-10 for a in range(3))
            for x, y, z in points:
                if not all(math.isfinite(v) for v in (x, y, z)):
                    raise ValueError("nonfinite surface sample")
                error = (min(abs(abs(x)-5.), abs(abs(y)-5.), abs(z), abs(z-10.)) if planar
                         else abs(math.sqrt(x*x + y*y + (z-10.)**2) - 2.))
                if error >= 1e-8:
                    raise ValueError("surface sample leaves analytic surface")
                surface_maximum, surface_count = max(surface_maximum, error), surface_count + 1
        if not value["topology"]["solid"]:
            raise ValueError("native artifact is not solid")
        rows.append(dict(id=name, solid=True, faces=len(value["faces"]), edges=len(value["edges"]),
                         edge_samples=count, maximum_boundary_sample_error=maximum,
                         surface_samples=surface_count, maximum_surface_sample_error=surface_maximum,
                         display_mesh=value["mesh"]))
    if len(rows) != 3 or len({r["id"] for r in rows}) != 3:
        raise ValueError("three distinct artifacts required")
    result = dict(engine=native["engine_version"], cases=rows, geometric_epsilon=1e-8,
                  scope="Rhino accepts each 3DM as a valid solid; samples qualify analytic boundary/surface membership, not full interactive command parity.")
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    print("Qualified 3 artifacts, %d edge and %d surface samples" % (
        sum(r["edge_samples"] for r in rows), sum(r["surface_samples"] for r in rows)))


if __name__ == "__main__":
    main()
