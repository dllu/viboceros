"""Independent high-precision area reference; requires optional mpmath.

This global-basis quadrature does not call either geometry kernel. It is an
accuracy witness, not a formally verified error enclosure.
"""
import argparse
import json
from pathlib import Path

import mpmath as mp


def extrusion_area(surface):
    """Integrate the first U control row's speed, times extrusion height 3."""
    degree = surface["degree_u"]
    count = surface["control_point_count_u"]
    knots = list(map(mp.mpf, surface["knots_u"]))
    controls = surface["control_points"][:count]
    points = [[mp.mpf(x) for x in control["point"]] for control in controls]
    weights = [mp.mpf(control["weight"]) for control in controls]

    def basis(t, order):
        values = [
            mp.mpf(int(
                (knots[i] <= t < knots[i + 1])
                or (t == knots[count] and i == count - 1)
            ))
            for i in range(len(knots) - 1)
        ]
        for d in range(1, order + 1):
            next_values = []
            for i in range(len(values) - 1):
                value = mp.mpf(0)
                if knots[i + d] != knots[i]:
                    value += (t - knots[i]) * values[i] / (knots[i + d] - knots[i])
                if knots[i + d + 1] != knots[i + 1]:
                    value += ((knots[i + d + 1] - t) * values[i + 1]
                              / (knots[i + d + 1] - knots[i + 1]))
                next_values.append(value)
            values = next_values
        return values

    def speed(t):
        values = basis(t, degree)
        lower = basis(t, degree - 1)
        derivatives = []
        for i in range(count):
            derivative = mp.mpf(0)
            if knots[i + degree] != knots[i]:
                derivative += degree * lower[i] / (knots[i + degree] - knots[i])
            if knots[i + degree + 1] != knots[i + 1]:
                derivative -= (degree * lower[i + 1]
                               / (knots[i + degree + 1] - knots[i + 1]))
            derivatives.append(derivative)
        weight = mp.fsum(values[i] * weights[i] for i in range(count))
        weight_derivative = mp.fsum(derivatives[i] * weights[i] for i in range(count))
        jet = []
        for axis in range(3):
            numerator = mp.fsum(values[i] * weights[i] * points[i][axis]
                                for i in range(count))
            derivative = mp.fsum(derivatives[i] * weights[i] * points[i][axis]
                                for i in range(count))
            jet.append((derivative * weight - numerator * weight_derivative) / weight**2)
        return mp.sqrt(mp.fsum(x * x for x in jet))

    boundaries = sorted(set(knots[degree:count + 1]))
    return 3 * mp.quad(speed, boundaries)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--digits", type=int, default=80)
    args = parser.parse_args()
    if not 30 <= args.digits <= 500:
        parser.error("digits must lie between 30 and 500")
    request_path = Path(__file__).resolve().parents[1] / "fixtures" / "join_trim_certificates.json"
    request = json.loads(request_path.read_text())
    with mp.workdps(args.digits):
        answer = {}
        for name in ("quadratic", "rational-cubic", "unclamped", "multispan"):
            operation = next(o for o in request["operations"]
                             if o["id"] == name + "-uv0-origin0-Join-pre")
            surface = operation["sources"][0]["brep"]["source"]["surface"]
            area = extrusion_area(surface)
            if name == "quadratic":
                analytic = 45 * (mp.sqrt(2) + mp.asinh(1))
                assert abs(area - analytic) < mp.power(10, -args.digits + 8)
            answer[name] = mp.nstr(area, args.digits - 5)
        print(json.dumps(answer, indent=2))


if __name__ == "__main__":
    main()
