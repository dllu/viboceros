"""Compare coaxial coincident-box Intersect output by its exact edge geometry.

Rhino and Viboceros may join the same twelve overlap-box edges into different
curve paths. This audit checks the result count and the undirected edge set.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

from tools.rhino_oracle.client import OracleClient


def expected_edges(operation: dict) -> list[tuple[tuple[float, ...], tuple[float, ...]]]:
    first_lower = operation["first_box_min"]
    first_upper = operation["first_box_max"]
    second_lower = operation["second_box_min"]
    second_upper = operation["second_box_max"]
    if any(
        first_lower[axis] != second_lower[axis] or first_upper[axis] != second_upper[axis]
        for axis in (0, 1)
    ):
        raise ValueError("the fixture requires coincident X and Y box intervals")
    lower = [max(left, right) for left, right in zip(first_lower, second_lower)]
    upper = [min(left, right) for left, right in zip(first_upper, second_upper)]
    if any(left >= right for left, right in zip(lower, upper)):
        raise ValueError("the fixture requires a three-dimensional box overlap")
    edges = []
    for axis in range(3):
        other = [coordinate for coordinate in range(3) if coordinate != axis]
        for first_fixed in (lower[other[0]], upper[other[0]]):
            for second_fixed in (lower[other[1]], upper[other[1]]):
                start = list(lower)
                end = list(lower)
                end[axis] = upper[axis]
                start[other[0]] = end[other[0]] = first_fixed
                start[other[1]] = end[other[1]] = second_fixed
                edges.append((tuple(start), tuple(end)))
    return edges


def edge_error(actual: tuple, expected: tuple) -> float:
    def point_error(first: tuple, second: tuple) -> float:
        return max(abs(left - right) for left, right in zip(first, second))

    return min(
        max(point_error(actual[0], expected[0]), point_error(actual[1], expected[1])),
        max(point_error(actual[0], expected[1]), point_error(actual[1], expected[0])),
    )


def audit_result(operation: dict, result: dict, epsilon: float) -> dict:
    value = result["value"]
    objects = value["objects"]
    if not value["command_succeeded"] or not objects:
        raise ValueError("coincident-box Intersect must create edge curves")
    actual = []
    for item in objects:
        if item["kind"] != "curve" or item["curve"]["degree"] != 1:
            raise ValueError("coincident-box Intersect must create linear curves")
        controls = [tuple(control["point"]) for control in item["curve"]["control_points"]]
        actual.extend(zip(controls, controls[1:]))
    expected = expected_edges(operation)
    if len(actual) != len(expected):
        raise ValueError(f"expected {len(expected)} edges, got {len(actual)}")
    remaining = list(actual)
    maximum_error = 0.0
    for target in expected:
        index = min(range(len(remaining)), key=lambda candidate: edge_error(remaining[candidate], target))
        maximum_error = max(maximum_error, edge_error(remaining.pop(index), target))
    if maximum_error > epsilon:
        raise ValueError(f"box edge difference {maximum_error} exceeds {epsilon}")
    return {"curves": len(objects), "edges": len(actual), "max_coordinate_error": maximum_error}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("fixture", type=Path)
    parser.add_argument("observations", type=Path)
    parser.add_argument("--native", type=Path, help="saved native response; otherwise run the native probe")
    parser.add_argument("--epsilon", type=float, default=1e-10)
    parser.add_argument("--timeout", type=float, default=300.0)
    args = parser.parse_args()
    request = json.loads(args.fixture.read_text())
    rhino = json.loads(args.observations.read_text())
    native = (
        json.loads(args.native.read_text())
        if args.native is not None
        else OracleClient().run_viboceros(request, args.timeout)
    )
    operations = {operation["id"]: operation for operation in request["operations"]}
    report = {}
    for engine, response in (("rhino", rhino), ("viboceros", native)):
        results = {result["id"]: result for result in response["results"]}
        report[engine] = {
            identifier: audit_result(operation, results[identifier], args.epsilon)
            for identifier, operation in operations.items()
        }
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
