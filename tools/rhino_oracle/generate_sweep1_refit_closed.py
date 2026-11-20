"""Capture adaptive fits of closed Sweep1 profiles."""
from copy import deepcopy
from math import cos, sin

from .capture_family import capture
from .generate_sweep1_refit import request as base_request
from .generate_sweep1_refit_holdouts import request as holdouts


def request():
    root = base_request()
    templates = {op["id"]: op for op in root["operations"] + holdouts()["operations"]}
    operations = []

    def add(name, template, tolerance, command=False):
        op = deepcopy(templates[template])
        op.update(id=name, refit_tolerance=tolerance, command=command, refit_rail=False)
        operations.append(op)
        return op

    add("circle-tight", "closed-refit-0.001", 0.0002, command=True)
    add("circle-second-pass", "closed-refit-0.001", 0.0005)
    add("rational-circle-tight", "nurbs-circle-0.001", 0.0002)
    rotated = add("rotated-circle-tight", "closed-refit-0.001", 0.00062)
    rotated["sections"][0].update(radius=7.3, x_axis=[cos(0.37), sin(0.37), 0.0])
    add("unequal-circles-tight", "two-closed-refit-0.001", 0.0004)
    add("cubic-loop-command", "closed-cubic-0.001", 0.001, command=True)
    root["operations"] = operations
    return root


if __name__ == "__main__":
    capture(
        "sweep1_refit_closed",
        request(),
        [
            "generate_sweep1_refit_closed.py",
            "generate_sweep1_refit_holdouts.py",
            "generate_sweep1_refit.py",
            "generate_sweep1_rebuild.py",
            "generate_sweep1_rebuild_holdouts.py",
            "sweep1_probe.py",
            "rhino_worker.py",
            "client.py",
            "structural_sdk.py",
        ],
        "VibocerosOracleSweepRefitClosed20261120",
        timeout=300,
    )
