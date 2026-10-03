"""Bounded public TwistSpaceMorph point samples, independent of command output."""

import math
import re


def validate(op):
    if (
        not isinstance(op, dict)
        or set(op)
        != {"op", "id", "axis_start", "axis_end", "angle", "infinite", "points"}
        or op["op"] != "twist_points"
        or not isinstance(op["id"], str)
        or re.match(r"^[A-Za-z0-9_-]{1,80}\Z", op["id"]) is None
        or type(op["infinite"]) is not bool
        or type(op["angle"]) not in (int, float)
        or (math.isnan(op["angle"]) or math.isinf(op["angle"]))
        or not isinstance(op["points"], list)
        or not 1 <= len(op["points"]) <= 256
    ):
        raise ValueError("invalid twist point probe")
    for p in [op["axis_start"], op["axis_end"]] + op["points"]:
        if (
            not isinstance(p, list)
            or len(p) != 3
            or any(
                type(v) not in (int, float) or (math.isnan(v) or math.isinf(v))
                for v in p
            )
        ):
            raise ValueError("twist samples require finite points")
    if op["axis_start"] == op["axis_end"]:
        raise ValueError("twist axis must differ")


def request():
    cases = []
    for infinite in (False, True):
        for degrees in (0.0, 90.0, -90.0, 720.0, 1e-8):
            cases.append(
                dict(
                    op="twist_points",
                    id="twist-z-%s-%s"
                    % (
                        "infinite" if infinite else "finite",
                        str(degrees).replace(".", "_").replace("-", "minus"),
                    ),
                    axis_start=[0.0, 0.0, 0.0],
                    axis_end=[0.0, 0.0, 10.0],
                    angle=math.radians(degrees),
                    infinite=infinite,
                    points=[
                        [2.0, 1.0, z]
                        for z in [
                            -10.0,
                            -2.0,
                            0.0,
                            0.25,
                            2.5,
                            5.0,
                            7.5,
                            9.75,
                            10.0,
                            12.0,
                            20.0,
                        ]
                    ]
                    + [[0.0, 0.0, z] for z in [0.0, 5.0, 10.0]],
                )
            )
        for start, end in [
            ([0.0, 0.0, 10.0], [0.0, 0.0, 0.0]),
            ([1.0, 2.0, 3.0], [5.0, 6.0, 11.0]),
        ]:
            cases.append(
                dict(
                    op="twist_points",
                    id="twist-%s-%s"
                    % (
                        "reverse" if start[0] == 0 else "spatial",
                        "infinite" if infinite else "finite",
                    ),
                    axis_start=start,
                    axis_end=end,
                    angle=math.pi / 3,
                    infinite=infinite,
                    points=[
                        [2.0, -1.0, z]
                        for z in [-2.0, 0.0, 3.0, 5.0, 8.0, 10.0, 12.0, 20.0]
                    ],
                )
            )
    return dict(protocol_version=1, iterations=1, operations=cases)


def run(op, host, iterations):
    validate(op)
    Rhino = host["Rhino"]
    morph = Rhino.Geometry.Morphs.TwistSpaceMorph()
    try:
        morph.TwistAxis = Rhino.Geometry.Line(
            host["_point"](op["axis_start"]), host["_point"](op["axis_end"])
        )
        morph.TwistAngleRadians = float(op["angle"])
        morph.InfiniteTwist = op["infinite"]
        morph.QuickPreview = False
        points = [host["_point"](p) for p in op["points"]]
        result, elapsed = host["_measure"](
            iterations, lambda: [host["_xyz"](morph.MorphPoint(p)) for p in points]
        )
        return dict(points=result), elapsed
    finally:
        morph.Dispose()
