"""Compare six-sample wire records only when nearest correspondences are bijective.

This is a bounded diagnostic, not a general curve/Hausdorff matching algorithm.
Run with python3 -m tools.numerics.compare_surface_wire_records NATIVE RHINO.
"""
import argparse
from copy import deepcopy
import json
import math
from pathlib import Path

from tools.rhino_oracle.client import compare_responses


def _coordinates(curve):
    if not isinstance(curve, list) or len(curve) != 6:
        raise ValueError("wire records require six sample points")
    values = []
    for point in curve:
        if not isinstance(point, list) or len(point) != 3:
            raise ValueError("wire samples require three coordinates")
        for value in point:
            if isinstance(value, bool) or not isinstance(value, (float, int)) or not math.isfinite(value):
                raise ValueError("wire coordinates must be finite numbers")
            values.append(value)
    return values


def align_wires(native, reference):
    """Reorder without editing coordinates; refuse unequal or ambiguous sets."""
    if len(native) != len(reference):
        raise ValueError("wire counts differ; no bijective correspondence claimed")
    a, b = [[_coordinates(curve) for curve in curves] for curves in [native, reference]]
    order = []
    for curve in a:
        costs = [max(abs(x-y) for x, y in zip(curve, candidate)) for candidate in b]
        best = min(costs)
        if not math.isfinite(best) or costs.count(best) != 1:
            raise ValueError("ambiguous nearest wire; no correspondence claimed")
        order.append(costs.index(best))
    if len(set(order)) != len(order):
        raise ValueError("nearest wires are not bijective; no correspondence claimed")
    return [reference[i] for i in order], order


def compare(native, reference, absolute=1e-9, relative=1e-12):
    # First validate the complete public response protocol, without changing it.
    compare_responses(native, reference, absolute, relative)
    aligned = deepcopy(reference)
    reference_results = {r["id"]:r for r in aligned["results"]}
    correspondences = []
    for record in native["results"]:
        other = reference_results[record["id"]]
        fields = {}
        for key in ["surface_wires", "brep_wires"]:
            other["value"][key], fields[key] = align_wires(record["value"][key], other["value"][key])
        correspondences.append(dict(id=record["id"], reference_wire_indices=fields))
    result = compare_responses(native, aligned, absolute, relative).as_dict()
    result["correspondences"] = correspondences
    result["matching_note"] = "Unique nearest six-sample max-coordinate matches form a bijection in each wire set. Only reference array order changes; every sample and topology field is then checked by the standard comparator. This is not a general curve-distance or topology proof."
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("native", type=Path)
    parser.add_argument("reference", type=Path)
    args = parser.parse_args()
    report = compare(json.loads(args.native.read_text()), json.loads(args.reference.read_text()))
    print(json.dumps(report, indent=2, allow_nan=False))
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
