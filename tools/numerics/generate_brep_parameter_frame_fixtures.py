#!/usr/bin/env python3
"""Print paired local/translated UV mass fixtures with dyadic trim coordinates."""
from copy import deepcopy
import json
import math
from pathlib import Path


def main():
    root = Path(__file__).resolve().parents[2]
    source = json.loads((root / "tools/rhino_oracle/fixtures/trimmed_mass_properties.json").read_text())
    operations = []
    configurations = [
        ("disk-u", 0, [0.5], [1e12, 0.0]),
        ("disk-v", 0, [0.5], [0.0, -2e12]),
        ("annulus", 1, [0.5, 0.25], [1e12, -2e12]),
        ("capped-outward", 3, [0.5], [1e12, -2e12]),
        ("capped-inward", 4, [0.5], [-1e12, 2e12]),
    ]
    for name, index, radii, translation in configurations:
        definition = deepcopy(source["operations"][index])
        for boundary, radius in zip(definition["boundaries"], radii):
            for key in ["curve", "parameter_curve"]:
                original_radius = max(abs(c["point"][axis])
                                      for c in boundary[key]["control_points"] for axis in range(2))
                for control in boundary[key]["control_points"]:
                    x, y, _ = control["point"]
                    control["point"] = [
                        math.copysign(radius, x) if abs(x) > original_radius * 0.5 else 0.0,
                        math.copysign(radius, y) if abs(y) > original_radius * 0.5 else 0.0,
                        radius * radius if key == "curve" else 0.0,
                    ]
        if definition.get("cap_surface"):
            for control in definition["cap_surface"]["control_points"]:
                control["point"][2] = radii[0] ** 2
        definition["interior_uv"] = [0.375 if len(radii) == 2 else 0.0, 0.0]
        for label, offset in [("local", [0.0, 0.0]), ("translated", translation)]:
            operation = deepcopy(definition)
            operation["id"] = f"parameter-frame-{name}-{label}"
            surfaces = [operation["surface"]]
            if operation.get("cap_surface"):
                surfaces.append(operation["cap_surface"])
            for surface in surfaces:
                for axis, key in enumerate(["knots_u", "knots_v"]):
                    surface[key] = [k + offset[axis] for k in surface[key]]
            for boundary in operation["boundaries"]:
                for control in boundary["parameter_curve"]["control_points"]:
                    for axis in range(2):
                        control["point"][axis] += offset[axis]
            operation["interior_uv"] = [p + shift for p, shift in zip(operation["interior_uv"], offset)]
            operations.append(operation)
    print(json.dumps(dict(protocol_version=1, iterations=3,
                          tolerance=source["tolerance"], operations=operations), indent=2))


if __name__ == "__main__":
    main()
