"""Capture matched Sweep1 sections with point ends."""
from copy import deepcopy

from .capture_family import capture
from .generate_sweep1_refit_polygon_profiles import request as polygon_request


def request():
    root = polygon_request()
    templates = {op["id"]: op for op in root["operations"]}
    operations = []

    def add(name, template, start, end, command=False):
        op = deepcopy(templates[template])
        stations = [2.5 if start else 0.0, 7.5 if end is not None else 10.0]
        op.update(id=name, parameters=stations, command=command, refit_rail=True)
        op["rail"]["end"][2] = 10.0
        if start:
            op["start_point"] = [0.0, 0.0, 0.0]
        if end is not None:
            op["end_point"] = end
        for section, z in zip(op["sections"], stations):
            if "center" in section:
                section["center"][2] = z
            elif "vertices" in section:
                for point in section["vertices"]:
                    point[2] = z
            else:
                for control in section["control_points"]:
                    control["point"][2] = z
        operations.append(op)

    on_rail = [0.0, 0.0, 10.0]
    add("polygon-loop-start-pole", "closed-polygon-loop", True, None)
    add("polygon-loop-offset-pole", "closed-polygon-loop", False, [1.5, -0.5, 10.0])
    add("polygon-loop-both-poles", "closed-polygon-loop", True, on_rail)
    add("polygon-loop-both-poles-command", "closed-polygon-loop", True, on_rail, command=True)
    add("polygon-ellipse-both-poles", "closed-polygon-ellipse-tight", True, on_rail)
    add("polygon-circle-both-poles", "closed-polygon-circle", True, on_rail)
    add("polygon-circle-both-poles-command", "closed-polygon-circle-command", True, on_rail, command=True)
    root["operations"] = operations
    return root


if __name__ == "__main__":
    capture(
        "sweep1_refit_polygon_poles",
        request(),
        [
            "generate_sweep1_refit_polygon_poles.py",
            "generate_sweep1_refit_polygon_profiles.py",
            "generate_sweep1_refit_loop_pairs.py",
            "generate_sweep1_refit.py",
            "generate_sweep1_rebuild.py",
            "sweep1_probe.py",
            "rhino_worker.py",
            "client.py",
            "structural_sdk.py",
        ],
        "VibocerosOracleSweepRefitPolygonPoles20261120",
        timeout=300,
    )
