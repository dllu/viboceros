"""Capture Sweep1 refits of straight and cornered cross sections."""
from copy import deepcopy

from .capture_family import capture
from .generate_sweep1_refit import request as base_request
from .generate_sweep1_refit_loop_pairs import circle, cubic, ellipse


def request():
    root = base_request()
    templates = {op["id"]: op for op in root["operations"]}
    template = templates["two-closed-refit-0.001"]
    open_curve = templates["two-open-refit-0.001"]["sections"][0]
    operations = []

    def line(z):
        return dict(type="line", start=[0.0, 0.0, z], end=[3.0, 0.0, z])

    def polygon(z, closed=False, align=False):
        if align:
            xy = [(2, 0), (2, 1), (0, 1), (0, 0), (2, 0)]
        elif closed:
            xy = [(0, 0), (2, 0), (2, 1), (0, 1), (0, 0)]
        else:
            xy = [(0, 0), (2, 2), (3, 0)]
        return dict(type="polyline", vertices=[[float(x), float(y), z] for x, y in xy])

    def add(name, sections, distance=0.001, command=False):
        op = deepcopy(template)
        stations = [5.0 * i / (len(sections) - 1) for i in range(len(sections))]
        op.update(id=name, sections=sections, parameters=stations,
                  command=command, refit_rail=False, refit_tolerance=distance)
        operations.append(op)

    add("line-loop", [line(0.0), cubic(5.0)])
    add("line-ellipse", [line(0.0), ellipse(5.0)])
    add("line-circle", [line(0.0), circle(5.0)])
    add("open-polygon-loop", [polygon(0.0), cubic(5.0)], 0.01)
    add("open-polygon-ellipse", [polygon(0.0), ellipse(5.0)])
    add("open-polygon-circle", [polygon(0.0), circle(5.0)])
    add("closed-polygon-loop", [polygon(0.0, closed=True), cubic(5.0)])
    add("closed-polygon-ellipse", [polygon(0.0, closed=True), ellipse(5.0)], 0.01)
    add("closed-polygon-ellipse-tight", [polygon(0.0, closed=True), ellipse(5.0)])
    add("closed-polygon-circle", [polygon(0.0, closed=True), circle(5.0)])
    add("open-curve-polygon", [open_curve, polygon(5.0)])
    add("line-polygon", [line(0.0), polygon(5.0)], 0.01)
    add("polygon-loop-circle", [polygon(0.0), cubic(2.5), circle(5.0)])
    add("line-polygon-ellipse", [line(0.0), polygon(2.5, closed=True), ellipse(5.0)])
    add("curve-polygon-loop", [open_curve, polygon(2.5), cubic(5.0)], 0.01)
    add("polygon-ellipse-circle", [polygon(0.0, closed=True), ellipse(2.5), circle(5.0)], 0.01)
    add("closed-polygon-loop-command", [polygon(0.0, closed=True), cubic(5.0)], 0.01, command=True)
    add("closed-polygon-loop-command-tight", [polygon(0.0, closed=True), cubic(5.0)], command=True)
    add("closed-polygon-circle-command", [polygon(0.0, align=True), circle(5.0)], command=True)
    root["operations"] = operations
    return root


if __name__ == "__main__":
    capture(
        "sweep1_refit_polygon_profiles",
        request(),
        [
            "generate_sweep1_refit_polygon_profiles.py",
            "generate_sweep1_refit_loop_pairs.py",
            "generate_sweep1_refit.py",
            "generate_sweep1_rebuild.py",
            "sweep1_probe.py",
            "rhino_worker.py",
            "client.py",
            "structural_sdk.py",
        ],
        "VibocerosOracleSweepRefitPolygons20261120",
        timeout=300,
    )
