#!/usr/bin/env python3
"""Print local/translated isocurve probes from the dyadic paraboloid fixtures."""
from copy import deepcopy
import json
from pathlib import Path


def main():
    root = Path(__file__).resolve().parents[2]
    request = json.loads((root / "tools/rhino_oracle/fixtures/brep_parameter_frames.json").read_text())
    operations = []
    for original in request["operations"]:
        if original.get("cap_surface"):
            continue
        operation = deepcopy(original)
        operation["op"] = "trimmed_surface_isocurves"
        operation["id"] = original["id"].replace("parameter-frame", "isocurve-frame")
        # The source surfaces have domain [-1, 1] before translation.
        origin = [operation["surface"][key][0] + 1. for key in ["knots_u", "knots_v"]]
        operation["parameters"] = [[u + origin[0], v + origin[1]] for u, v in [
            [0.125, 0.375], [0.375, 0.125], [0., 0.], [0.75, 0.75]
        ]]
        operations.append(operation)
    request["operations"] = operations
    print(json.dumps(request, indent=2))


if __name__ == "__main__":
    main()
