"""Check fixed STEP pole fixtures imported through the licensed Rhino SDK."""
import argparse
import json
import math
from pathlib import Path


def boundary_error(case, point):
    x, y, z = point
    if not all(math.isfinite(v) for v in point):
        raise ValueError("nonfinite boundary sample")
    if case == "sphere":
        return abs(math.sqrt(x*x + y*y + z*z) - 2.)
    if case == "cone":
        radial = math.hypot(x, y)
        side = abs(radial - 2.*(1.-z/5.))/math.sqrt(1.16)
        if z < -1e-8 or z > 5.+1e-8 or radial > 2.+1e-8:
            raise ValueError("sample leaves cone bounds")
        return min(side, abs(z) if radial <= 2.+1e-8 else math.inf)
    if any(v < -1e-8 or v > 10.+1e-8 for v in point):
        raise ValueError("sample leaves box bounds")
    center_z = 10. if case == "sphere-pocket" else 5.
    sphere = abs(math.sqrt((x-5.)**2 + (y-5.)**2 + (z-center_z)**2) - 2.)
    box = min(min(abs(v), abs(v-10.)) for v in point)
    return min(sphere, box)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("native", type=Path)
    parser.add_argument("properties", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    native = json.loads(args.native.read_text())
    properties = {r["id"]: r for r in json.loads(args.properties.read_text())}
    cases = ("sphere", "cone", "sphere-pocket", "sphere-cavity")
    results = native["results"]
    if len(results) != 4 or {r["id"] for r in results} != set(cases):
        raise ValueError("expected four unique pole fixtures")
    rows = []
    for record in results:
        name, value = record["id"], record["value"]
        objects = value["objects"]
        if not value["imported"] or len(objects) != (2 if name == "sphere-cavity" else 1):
            raise ValueError("unexpected native import or object count")
        maximum, count, singular = 0., 0, 0
        for obj in objects:
            if not obj["valid"] or not obj["solid"]:
                raise ValueError("invalid or open native B-rep")
            geometry = obj["geometry"]
            samples = [p for edge in geometry["edges"] for p in edge["curve"]["samples"]]
            samples += [p for face in geometry["faces"] for loop in face["loops"]
                        for trim in loop for p in trim["lifted"]]
            for point in samples:
                error = boundary_error(name, point)
                if error > 1e-8:
                    raise ValueError("boundary sample exceeds geometric epsilon")
                maximum, count = max(maximum, error), count+1
            singular += sum(trim["type"] == "Singular"
                            for face in geometry["topology"]["faces"]
                            for loop in face["loops"] for trim in loop["trims"])
        if count == 0 or singular == 0:
            raise ValueError("missing boundary or pole observations")
        qualified_solid = name != "sphere-cavity"
        relative_volume_error = None
        if qualified_solid:
            expected, actual = properties[name]["volume"], objects[0]["volume"]
            if actual is None or not math.isfinite(actual):
                raise ValueError("missing native volume")
            relative_volume_error = abs(actual-expected)/abs(expected)
            if relative_volume_error > 2e-8:
                raise ValueError("native volume differs from source")
        rows.append(dict(id=name, native_objects=len(objects), boundary_samples=count,
                         singular_trims=singular, maximum_boundary_error=maximum,
                         qualified_solid_transfer=qualified_solid,
                         relative_volume_error=relative_volume_error,
                         native_volumes=[o["volume"] for o in objects],
                         native_orientations=[o["orientation"] for o in objects]))
    output = dict(engine=native["engine_version"], geometric_epsilon=1e-8,
                  relative_volume_epsilon=2e-8, cases=rows,
                  limitation="Curved cavity shells import separately; void semantics are lost. "
                             "Boundary samples qualify geometry, not complete equivalence.")
    args.output.write_text(json.dumps(output, indent=2)+"\n")
    print("Qualified three solid transfers and %d boundary samples; cavity grouping remains open"
          % sum(r["boundary_samples"] for r in rows))


if __name__ == "__main__":
    main()
