"""Capture section refitting across a right-angle Sweep1 rail."""
from math import sqrt

from .capture_family import capture


def profile(kind, station):
    center = [(0., 0.), (5., 0.), (5., 5.)][station]
    diagonal = sqrt(.5)
    axis = [(0., 1.), (-diagonal, diagonal), (-1., 0.)][station]
    scale = 1. - .15 * station

    def point(u, v):
        return [center[0] + scale * u * axis[0],
                center[1] + scale * u * axis[1], scale * v]

    if kind == "line":
        return dict(type="line", start=point(0., 0.), end=point(1.5, .5))
    if kind == "polygon":
        return dict(type="polyline", vertices=[point(*p) for p in
                    [(0., 0.), (1., 0.), (1., .5), (0., .5), (0., 0.)]])
    local = [(0., 0.), (.5, 1.), (1.5, 0.)] if kind == "open" else [
        (0., 0.), (1., 1.), (-1., 1.), (0., 0.)]
    return dict(type="nurbs", degree=len(local) - 1,
                knots=[0.] * len(local) + [1.] * len(local),
                control_points=[dict(point=point(*p), weight=1.) for p in local])


def request():
    rail = dict(type="polyline", vertices=[[0., 0., 0.], [5., 0., 0.], [5., 5., 0.]])
    operations = []

    def add(name, kinds, tolerance, untrimmed=True, refit_rail=False):
        stations = {1: [0], 2: [0, 2], 3: [0, 1, 2]}[len(kinds)]
        operations.append(dict(
            op="sweep1", id=name, rail=rail,
            sections=[profile(kind, station) for kind, station in zip(kinds, stations)],
            parameters=[5. * station for station in stations], command=True,
            natural_seams=True, inspect_definition=True, refit_rail=refit_rail,
            untrimmed_miters=untrimmed, refit_tolerance=tolerance,
        ))

    add("line", ["line"], .001)
    add("open", ["open"], .01)
    add("open-tight", ["open"], .001)
    add("polygon", ["polygon"], .001)
    add("inferred", ["polygon", "loop"], .01)
    add("inferred-tight", ["polygon", "loop"], .001)
    add("authored", ["polygon", "loop", "loop"], .01)
    add("authored-tight", ["polygon", "loop", "loop"], .001)
    add("authored-trimmed", ["polygon", "loop", "loop"], .01, untrimmed=False)
    add("authored-refit-rail", ["polygon", "loop", "loop"], .01, refit_rail=True)
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-7, relative=1e-12, angular=1e-10),
                operations=operations)


if __name__ == "__main__":
    capture(
        "sweep1_refit_miters", request(),
        ["generate_sweep1_refit_miters.py", "sweep1_probe.py", "rhino_worker.py",
         "client.py", "structural_sdk.py"],
        "VibocerosOracleSweepRefitMiters20261120", timeout=300,
    )
