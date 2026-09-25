"""Audit the saved saddle-plane fixture's fitted curves against its exact locus.

The fixture's positive-weight saddle has an invertible XY parameter map. Newton
inverts that map independently of both engines' curve parameterizations.
"""

from __future__ import annotations

import argparse
import json
from math import isfinite
from pathlib import Path

from tools.rhino_oracle.audit_cylinder_sections import _evaluate, _parameters


EXPECTED = {
    "two-conic-branches": ("curve", 2),
    "clipped-positive-conic": ("curve", 1),
    "two-center-rulings": ("curve", 2),
    "two-corner-contacts": ("point", 2),
    "rational-two-conics": ("curve", 2),
}


def _surface_sample(surface: dict, u: float, v: float):
    controls = surface["control_points"]
    basis = [(1 - u) * (1 - v), u * (1 - v), (1 - u) * v, u * v]
    du = [-(1 - v), 1 - v, -v, v]
    dv = [-(1 - u), -u, 1 - u, u]
    numerator = [0.0, 0.0, 0.0]
    derivative_u = [0.0, 0.0, 0.0]
    derivative_v = [0.0, 0.0, 0.0]
    denominator = weight_u = weight_v = 0.0
    for control, coefficient, u_coefficient, v_coefficient in zip(
        controls, basis, du, dv
    ):
        weight = control["weight"]
        point = control["point"]
        denominator += coefficient * weight
        weight_u += u_coefficient * weight
        weight_v += v_coefficient * weight
        for axis in range(3):
            value = weight * point[axis]
            numerator[axis] += coefficient * value
            derivative_u[axis] += u_coefficient * value
            derivative_v[axis] += v_coefficient * value
    point = [value / denominator for value in numerator]
    tangent_u = [
        (derivative_u[axis] - point[axis] * weight_u) / denominator
        for axis in range(3)
    ]
    tangent_v = [
        (derivative_v[axis] - point[axis] * weight_v) / denominator
        for axis in range(3)
    ]
    return point, tangent_u, tangent_v


def _closest_surface_parameters(surface: dict, target: list[float]):
    u = (target[0] + 1.0) * 0.5
    v = (target[1] + 1.0) * 0.5
    for _ in range(24):
        point, tangent_u, tangent_v = _surface_sample(surface, u, v)
        dx = point[0] - target[0]
        dy = point[1] - target[1]
        determinant = tangent_u[0] * tangent_v[1] - tangent_v[0] * tangent_u[1]
        if determinant == 0.0:
            raise ValueError("singular bilinear parameter map")
        step_u = (dx * tangent_v[1] - dy * tangent_v[0]) / determinant
        step_v = (dy * tangent_u[0] - dx * tangent_u[1]) / determinant
        u -= step_u
        v -= step_v
        if max(abs(step_u), abs(step_v)) < 1e-14:
            break
    point, _, _ = _surface_sample(surface, u, v)
    distance = sum((left - right) ** 2 for left, right in zip(point, target)) ** 0.5
    return u, v, distance


def audit_case(operation: dict, result: dict):
    kind, expected_count = EXPECTED[operation["id"]]
    value = result["value"]
    objects = value["objects"]
    if not value["command_succeeded"] or len(objects) != expected_count:
        raise ValueError("unexpected Intersect result count")
    if any(item["kind"] != kind for item in objects):
        raise ValueError("unexpected Intersect object type")
    surface = operation["first"]
    plane = operation["second"]
    plane_z = plane["control_points"][0]["point"][2]
    x_bounds = [point["point"][0] for point in plane["control_points"]]
    y_bounds = [point["point"][1] for point in plane["control_points"]]
    max_surface_error = max_plane_error = max_outside_error = max_endpoint_error = 0.0
    for item in objects:
        curve = item.get("curve")
        samples = (
            (_evaluate(curve, parameter) for parameter in _parameters(curve))
            if curve is not None
            else iter([item["point"]])
        )
        for point in samples:
            u, v, surface_error = _closest_surface_parameters(surface, point)
            max_surface_error = max(max_surface_error, surface_error)
            max_plane_error = max(max_plane_error, abs(point[2] - plane_z))
            max_outside_error = max(
                max_outside_error,
                -u, u - 1.0, -v, v - 1.0,
                min(x_bounds) - point[0], point[0] - max(x_bounds),
                min(y_bounds) - point[1], point[1] - max(y_bounds),
            )
        if curve is not None:
            for parameter in curve["domain"]:
                point = _evaluate(curve, parameter)
                u, v, _ = _closest_surface_parameters(surface, point)
                max_endpoint_error = max(
                    max_endpoint_error,
                    min(abs(u), abs(u - 1.0), abs(v), abs(v - 1.0)),
                )
    if kind == "point":
        actual = sorted(tuple(item["point"]) for item in objects)
        expected = [(-1.0, -1.0, 1.0), (1.0, 1.0, 1.0)]
        max_endpoint_error = max(
            max(abs(a - b) for a, b in zip(left, right))
            for left, right in zip(actual, expected)
        )
    metrics = {
        "surface": max_surface_error,
        "plane": max_plane_error,
        "outside": max_outside_error,
        "endpoints": max_endpoint_error,
    }
    if not all(isfinite(value) for value in metrics.values()):
        raise ValueError("non-finite audit metric")
    return metrics


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("fixture", type=Path)
    parser.add_argument("observations", type=Path)
    parser.add_argument("--max-error", type=float, default=5e-6)
    args = parser.parse_args()
    fixture = json.loads(args.fixture.read_text())
    observations = json.loads(args.observations.read_text())
    result_by_id = {result["id"]: result for result in observations["results"]}
    report = {}
    for operation in fixture["operations"]:
        identifier = operation["id"]
        report[identifier] = audit_case(operation, result_by_id[identifier])
    print(json.dumps(report, indent=2, sort_keys=True))
    return int(any(
        value > args.max_error
        for metrics in report.values()
        for value in metrics.values()
    ))


if __name__ == "__main__":
    raise SystemExit(main())
