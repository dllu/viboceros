"""Independent high-precision areas for the chord-adjustment extrusion fixtures.

Optional dependency: mpmath. Run from the repository root:
    python3 -m tools.rhino_oracle.join_chord_witness --precision 100
This integrates Bernstein polynomials directly, without the native kernel or Rhino.
"""

import argparse
import json
import math
from pathlib import Path


def areas(precision):
    import mpmath as mp

    mp.mp.dps = precision
    path = Path(__file__).with_name("fixtures") / "join_chord_adjustment.json"
    request = json.loads(path.read_text())

    def scalar(value):
        numerator, denominator = float(value).as_integer_ratio()
        return mp.mpf(numerator) / denominator

    def bernstein(controls, parameter):
        degree = len(controls) - 1
        return [
            sum(
                math.comb(degree, i)
                * (1 - parameter) ** (degree - i)
                * parameter**i
                * point[axis]
                for i, point in enumerate(controls)
            )
            for axis in range(4)
        ]

    for operation in request["operations"]:
        if operation["command"] != "Join" or not operation["preselect"]:
            continue
        for source_index, source in enumerate(operation["sources"]):
            surface = source["brep"]["source"]["surface"]
            degree = surface["degree_u"]
            count = degree + 1
            assert surface["degree_v"] == 1
            assert surface["control_point_count_u"] == count
            assert surface["control_point_count_v"] == 2
            assert surface["knots_u"] == [0] * count + [4] * count
            assert surface["knots_v"] == [0, 0, 1, 1]
            row = surface["control_points"][:count]
            top = surface["control_points"][count:]
            vector = [scalar(top[0]["point"][i]) - scalar(row[0]["point"][i]) for i in range(3)]
            for a, b in zip(row, top):
                assert a["weight"] == b["weight"]
                assert [scalar(b["point"][i]) - scalar(a["point"][i]) for i in range(3)] == vector
            controls = [
                [scalar(c["point"][i]) * scalar(c["weight"]) for i in range(3)]
                + [scalar(c["weight"])]
                for c in row
            ]
            derivatives = [
                [degree * (controls[i + 1][j] - controls[i][j]) for j in range(4)]
                for i in range(degree)
            ]

            def speed(parameter):
                h = bernstein(controls, parameter)
                derivative = bernstein(derivatives, parameter)
                tangent = [(derivative[i] * h[3] - h[i] * derivative[3]) / h[3] ** 2 for i in range(3)]
                cross = [tangent[(i + 1) % 3] * vector[(i + 2) % 3] - tangent[(i + 2) % 3] * vector[(i + 1) % 3] for i in range(3)]
                return mp.sqrt(sum(x * x for x in cross))

            area = mp.quad(speed, [mp.mpf(i) / 8 for i in range(9)])
            yield {"id": operation["id"], "source": source_index, "area": mp.nstr(area, precision)}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--precision", type=int, default=100)
    args = parser.parse_args()
    if not 30 <= args.precision <= 1000:
        parser.error("precision must be between 30 and 1000 digits")
    print(json.dumps(list(areas(args.precision)), indent=2))
