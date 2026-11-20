"""Compare shared refits of noncircular Sweep1 profiles."""
from copy import deepcopy

from .capture_family import capture
from .generate_sweep1_refit import request as base_request


def cubic(z, scale=1.0):
    return dict(
        type="nurbs", degree=3, knots=[0.0] * 4 + [1.0] * 4,
        control_points=[
            dict(point=[x * scale, y * scale, z], weight=1.0)
            for x, y in [(0.0, 0.0), (2.0, 2.0), (-2.0, 2.0), (0.0, 0.0)]
        ],
    )


def ellipse(z, scale=1.0):
    return dict(
        type="ellipse", center=[0.0, 0.0, z], radius_x=2.0 * scale, radius_y=1.2 * scale,
        x_axis=[1.0, 0.0, 0.0], y_axis=[0.0, 1.0, 0.0],
    )


def circle(z, scale=1.0):
    return dict(
        type="circle", center=[0.0, 0.0, z], radius=2.0 * scale,
        x_axis=[1.0, 0.0, 0.0], normal=[0.0, 0.0, 1.0],
    )


def request():
    root = base_request()
    template = next(op for op in root["operations"] if op["id"] == "two-closed-refit-0.001")
    operations = []

    def add(name, sections, limit, command=False):
        op = deepcopy(template)
        stations = [section["center"][2] if "center" in section else
                    section["control_points"][0]["point"][2] for section in sections]
        op.update(id=name, sections=sections, parameters=stations,
                  refit_tolerance=limit, command=command, refit_rail=False)
        op["rail"]["end"][2] = max(5.0, stations[-1])
        operations.append(op)

    add("cubic-loops-command", [cubic(0.0), cubic(5.0)], 0.01, command=True)
    add("scaled-cubic-loop-command", [cubic(0.0), cubic(5.0, 0.65)], 0.001, command=True)
    add("scaled-cubic-loop-tight", [cubic(0.0), cubic(5.0, 0.65)], 0.0003)
    add("ellipses", [ellipse(0.0), ellipse(5.0)], 0.01)
    add("scaled-ellipse-command", [ellipse(0.0), ellipse(5.0, 0.65)], 0.001, command=True)
    add("scaled-ellipse-tight", [ellipse(0.0), ellipse(5.0, 0.65)], 0.0003)
    add("cubic-and-ellipse", [cubic(0.0), ellipse(5.0)], 0.01)
    add("cubic-and-scaled-ellipse", [cubic(0.0), ellipse(5.0, 0.65)], 0.001)
    add("ellipse-and-circle-command", [ellipse(0.0), circle(5.0)], 0.001, command=True)
    add("ellipse-and-small-circle", [ellipse(0.0), circle(5.0, 0.65)], 0.01)
    add("single-ellipse-command", [ellipse(0.0)], 0.001, command=True)
    add("three-cubic-loops", [cubic(0.0), cubic(3.0, 0.65), cubic(8.0, 0.8)], 0.01)
    root["operations"] = operations
    return root


if __name__ == "__main__":
    capture(
        "sweep1_refit_loop_pairs",
        request(),
        [
            "generate_sweep1_refit_loop_pairs.py",
            "generate_sweep1_refit.py",
            "generate_sweep1_rebuild.py",
            "sweep1_probe.py",
            "rhino_worker.py",
            "client.py",
            "structural_sdk.py",
        ],
        "VibocerosOracleSweepRefitLoopPairs20261120",
        timeout=300,
    )
