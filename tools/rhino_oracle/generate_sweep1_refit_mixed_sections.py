"""Capture smooth refits with mixed closure and no retained curvature regions."""
from copy import deepcopy

from .capture_family import capture
from .generate_sweep1_refit import request as base_request
from .generate_sweep1_refit_loop_pairs import cubic, ellipse


def request():
    root = base_request()
    template = next(op for op in root["operations"] if op["id"] == "two-open-refit-0.001")
    operations = []

    def add(name, sections, distance):
        op = deepcopy(template)
        op.update(id=name, sections=sections, refit_tolerance=distance)
        operations.append(op)

    def pair(minor):
        sections = [ellipse(0.0), ellipse(5.0, 0.65)]
        for section, scale in zip(sections, [1.0, 0.65]):
            section["radius_y"] = minor * scale
        return sections

    add("unbroken-ellipses", pair(1.8), 0.01)
    add("near-circular-ellipses", pair(1.995), 0.001)
    add("unbroken-ellipses-tight", pair(1.95), 0.0004)
    add("open-and-cubic-loop", [template["sections"][0], cubic(5.0)], 0.001)
    add("open-and-ellipse", [template["sections"][0], ellipse(5.0)], 0.01)
    longer = deepcopy(template["sections"][0])
    for control in longer["control_points"]:
        point = control["point"]
        point[0] *= 2.0
        point[1] *= 2.0
    add("long-open-and-cubic-loop", [longer, cubic(5.0)], 0.01)
    root["operations"] = operations
    return root


if __name__ == "__main__":
    capture(
        "sweep1_refit_mixed_sections",
        request(),
        [
            "generate_sweep1_refit_mixed_sections.py",
            "generate_sweep1_refit_loop_pairs.py",
            "generate_sweep1_refit.py",
            "generate_sweep1_rebuild.py",
            "sweep1_probe.py",
            "rhino_worker.py",
            "client.py",
            "structural_sdk.py",
        ],
        "VibocerosOracleSweepRefitMixedSections20261120",
        timeout=300,
    )
