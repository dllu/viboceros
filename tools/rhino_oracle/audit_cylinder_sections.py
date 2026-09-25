"""Check saved Intersect curves against the exact plane/capped-cylinder locus.

Rhino's command fits cubic output for oblique cuts, so its NURBS controls and
parameter domain differ from Viboceros' exact rational section. This audit
checks the represented geometry without requiring either parameterization.
"""

from __future__ import annotations

import argparse
from bisect import bisect_right
import json
from math import isfinite, sqrt
from pathlib import Path


def _dot(left, right):
    return sum(a * b for a, b in zip(left, right))


def _subtract(left, right):
    return [a - b for a, b in zip(left, right)]


def _norm(vector):
    return sqrt(_dot(vector, vector))


def _evaluate(curve, parameter):
    """Evaluate the full-order rational B-spline saved by the oracle."""
    degree = curve["degree"]
    knots = curve["knots"]
    controls = curve["control_points"]
    span = min(max(bisect_right(knots, parameter) - 1, degree), len(controls) - 1)
    values = []
    for control in controls[span - degree : span + 1]:
        weight = control["weight"]
        values.append([*(weight * coordinate for coordinate in control["point"]), weight])
    for level in range(1, degree + 1):
        for local in range(degree, level - 1, -1):
            index = span - degree + local
            denominator = knots[index + degree - level + 1] - knots[index]
            alpha = (parameter - knots[index]) / denominator if denominator else 0.0
            values[local] = [
                (1.0 - alpha) * left + alpha * right
                for left, right in zip(values[local - 1], values[local])
            ]
    point = values[degree]
    return [coordinate / point[3] for coordinate in point[:3]]


def _parameters(curve):
    knots = curve["knots"]
    start, end = curve["domain"]
    breaks = sorted({start, end, *(knot for knot in knots if start < knot < end)})
    for left, right in zip(breaks, breaks[1:]):
        if right <= left:
            continue
        for index in range(64):
            yield left + (right - left) * index / 64.0
    yield end


def audit_case(operation, result):
    objects = result["value"]["objects"]
    if not result["value"]["command_succeeded"] or len(objects) != 1:
        raise ValueError("expected one successful closed section curve")
    item = objects[0]
    if item["kind"] != "curve":
        raise ValueError("expected a curve object")
    curve = item["curve"]
    patch = operation["surface"]["control_points"]
    origin = patch[0]["point"]
    first = _subtract(patch[1]["point"], origin)
    second = _subtract(patch[2]["point"], origin)
    cross = [
        first[1] * second[2] - first[2] * second[1],
        first[2] * second[0] - first[0] * second[2],
        first[0] * second[1] - first[1] * second[0],
    ]
    normal_length = _norm(cross)
    axis = operation["cylinder_axis"]
    axis_length = _norm(axis)
    if normal_length == 0.0 or axis_length == 0.0:
        raise ValueError("degenerate fixture plane or cylinder axis")
    normal = [value / normal_length for value in cross]
    axis = [value / axis_length for value in axis]
    center = operation["cylinder_center"]
    radius = operation["cylinder_radius"]
    height = operation["cylinder_height"]
    max_plane_error = max_boundary_error = max_outside_error = 0.0
    for parameter in _parameters(curve):
        point = _evaluate(curve, parameter)
        from_center = _subtract(point, center)
        axial = _dot(from_center, axis)
        radial = _norm([value - axial * direction for value, direction in zip(from_center, axis)])
        max_plane_error = max(max_plane_error, abs(_dot(_subtract(point, origin), normal)))
        max_boundary_error = max(
            max_boundary_error,
            min(abs(radial - radius), abs(axial), abs(axial - height)),
        )
        max_outside_error = max(
            max_outside_error, radial - radius, -axial, axial - height
        )
    closed_error = _norm(_subtract(_evaluate(curve, curve["domain"][0]),
                                   _evaluate(curve, curve["domain"][1])))
    values = {
        "plane": max_plane_error,
        "boundary": max_boundary_error,
        "outside": max_outside_error,
        "closure": closed_error,
    }
    if not all(isfinite(value) for value in values.values()):
        raise ValueError("non-finite curve sample")
    return values


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("fixture", type=Path)
    parser.add_argument("response", type=Path)
    parser.add_argument("--max-error", type=float, required=True)
    args = parser.parse_args()
    fixture = json.loads(args.fixture.read_text())
    response = json.loads(args.response.read_text())
    operations = {item["id"]: item for item in fixture["operations"]}
    results = {item["id"]: item for item in response["results"]}
    if operations.keys() != results.keys():
        parser.error("fixture and response case ids differ")
    passed = True
    for identifier, operation in operations.items():
        errors = audit_case(operation, results[identifier])
        worst = max(errors.values())
        passed &= worst <= args.max_error
        print(f"{identifier}: max error {worst:.6g}; {errors}")
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
