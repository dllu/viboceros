# -*- coding: utf-8 -*-
"""Bounded public-RhinoCommon camera probe for SetView CPlane directions."""
import math


DIRECTIONS = ("Top", "Bottom", "Front", "Back", "Right", "Left")
PROJECTIONS = ("Top", "Perspective")
try:
    string_types = (basestring,)
except NameError:
    string_types = (str,)


def _finite(value):
    return not (math.isnan(value) or math.isinf(value))


def _point(value, name):
    if not isinstance(value, list) or len(value) != 3:
        raise ValueError("%s must be a three-coordinate array" % name)
    if any(isinstance(item, bool) or not isinstance(item, (int, float))
           or not _finite(float(item)) for item in value):
        raise ValueError("%s must have finite numeric coordinates" % name)
    return [float(item) for item in value]


def validate(operation):
    if operation.get("op") != "view_camera_probe":
        raise ValueError("unsupported camera probe operation")
    for name in ("origin", "x_axis", "y_axis"):
        _point(operation.get(name), name)
    projections = operation.get("projections")
    directions = operation.get("directions")
    if not isinstance(projections, list) or not projections or len(projections) > 2 \
            or any(not isinstance(value, string_types) or value not in PROJECTIONS for value in projections) \
            or len(set(projections)) != len(projections):
        raise ValueError("camera probe projections must be distinct Top/Perspective values")
    if not isinstance(directions, list) or not directions or len(directions) > 6 \
            or any(not isinstance(value, string_types) or value not in DIRECTIONS for value in directions) \
            or len(set(directions)) != len(directions):
        raise ValueError("camera probe directions must be distinct standard CPlane views")


def script(direction):
    if direction not in DIRECTIONS:
        raise ValueError("unsupported CPlane view")
    return "_SetView _CPlane _" + direction


def _xyz(value):
    result = [float(value.X), float(value.Y), float(value.Z)]
    if any(not _finite(item) for item in result):
        raise ValueError("nonfinite camera or CPlane coordinate")
    return result


def _unit(value):
    components = _xyz(value)
    length = math.sqrt(sum(component * component for component in components))
    if not _finite(length) or length <= 0.0:
        raise ValueError("invalid camera direction or up vector")
    return [component / length for component in components]


def _snapshot(viewport, projection, direction):
    plane = viewport.ConstructionPlane()
    return dict(
        projection=projection, direction=direction,
        perspective=bool(viewport.IsPerspectiveProjection),
        camera_location=_xyz(viewport.CameraLocation),
        camera_target=_xyz(viewport.CameraTarget),
        camera_direction=_unit(viewport.CameraDirection),
        camera_up=_unit(viewport.CameraUp),
        cplane_origin=_xyz(plane.Origin),
        cplane_x=_unit(plane.XAxis),
        cplane_y=_unit(plane.YAxis),
    )


def run(operation, viewport, host):
    validate(operation)
    Rhino = host["Rhino"]
    origin = _point(operation["origin"], "origin")
    x_axis = _point(operation["x_axis"], "x_axis")
    y_axis = _point(operation["y_axis"], "y_axis")
    plane = Rhino.Geometry.Plane(
        Rhino.Geometry.Point3d(*origin),
        Rhino.Geometry.Vector3d(*x_axis),
        Rhino.Geometry.Vector3d(*y_axis),
    )
    if not plane.IsValid:
        raise ValueError("invalid CPlane frame")
    original = Rhino.DocObjects.ViewportInfo(viewport)
    original_target = viewport.CameraTarget
    original_name = viewport.Name
    try:
        results = []
        for projection in operation["projections"]:
            defined = getattr(Rhino.Display.DefinedViewportProjection, projection)
            for direction in operation["directions"]:
                if not viewport.SetProjection(defined, "SetView camera probe", False):
                    raise ValueError("could not set camera probe projection")
                if viewport.SetConstructionPlane(plane) is False:
                    raise ValueError("could not set camera probe CPlane")
                if not Rhino.RhinoApp.RunScript(script(direction), False):
                    raise ValueError("SetView CPlane command failed")
                results.append(_snapshot(viewport, projection, direction))
        return results
    finally:
        errors = []
        for label, action in [
            ("projection", lambda: viewport.SetViewProjection(original, False)),
            ("target", lambda: viewport.SetCameraTarget(original_target, False)),
            ("name", lambda: setattr(viewport, "Name", original_name)),
            ("viewport info", original.Dispose),
        ]:
            try:
                if action() is False:
                    raise ValueError("API returned false")
            except Exception as error:
                errors.append("%s: %s" % (label, error))
        if errors:
            raise ValueError("camera probe cleanup failed: " + "; ".join(errors))


def _cross(a, b):
    return [a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0]]


def _normalized(values):
    length = math.sqrt(sum(value * value for value in values))
    if not _finite(length) or length <= 0.0:
        raise ValueError("degenerate recorded camera frame")
    return [value / length for value in values]


def _maximum_difference(a, b):
    return max(abs(left - right) for left, right in zip(a, b))


def compare_to_viboceros(operation, rows, epsilon=1.0e-9):
    """Compare recorded public camera state with the implemented camera rule.

    The orientation and CPlane are compared componentwise. Parallel camera
    location is diagnostic because Viboceros represents parallel views without
    a finite camera location; perspective distance uses Viboceros's current 50
    model-unit default.
    """
    validate(operation)
    if not _finite(float(epsilon)) or epsilon < 0.0:
        raise ValueError("invalid camera comparison epsilon")
    expected_x = _normalized(_point(operation["x_axis"], "x_axis"))
    input_y = _point(operation["y_axis"], "y_axis")
    y_along_x = sum(a * b for a, b in zip(input_y, expected_x))
    expected_y = _normalized([a - y_along_x * b for a, b in zip(input_y, expected_x)])
    expected_pairs = [(projection, direction)
                      for projection in operation["projections"]
                      for direction in operation["directions"]]
    if not isinstance(rows, list) or len(rows) != len(expected_pairs):
        raise ValueError("camera probe did not return every requested view")
    results = []
    for row, (projection, direction) in zip(rows, expected_pairs):
        if row["projection"] != projection or row["direction"] != direction:
            raise ValueError("camera probe view order changed")
        vectors = {name: _point(row.get(name), name) for name in (
            "cplane_origin", "cplane_x", "cplane_y", "camera_direction",
            "camera_up", "camera_location", "camera_target")}
        x = _normalized(vectors["cplane_x"])
        y = _normalized(vectors["cplane_y"])
        z = _normalized(_cross(x, y))
        neg = lambda vector: [-value for value in vector]
        expected = {
            "Top": (neg(z), y),
            "Bottom": (z, neg(y)),
            "Front": (y, z),
            "Back": (neg(y), z),
            "Right": (neg(x), z),
            "Left": (x, z),
        }[direction]
        orientation_error = max(
            _maximum_difference(vectors["camera_direction"], expected[0]),
            _maximum_difference(vectors["camera_up"], expected[1]),
        )
        plane_error = _maximum_difference(vectors["cplane_origin"], operation["origin"])
        axes_error = max(
            _maximum_difference(vectors["cplane_x"], expected_x),
            _maximum_difference(vectors["cplane_y"], expected_y),
        )
        target_error = _maximum_difference(vectors["camera_target"], vectors["cplane_origin"])
        projection_matches = bool(row["perspective"]) == (projection == "Perspective")
        distance = math.sqrt(sum(
            (location - target) ** 2
            for location, target in zip(vectors["camera_location"], vectors["camera_target"])
        ))
        distance_error = abs(distance - 50.0) if projection == "Perspective" else None
        passed = (projection_matches and orientation_error <= epsilon
                  and plane_error <= epsilon and axes_error <= epsilon
                  and target_error <= epsilon
                  and (distance_error is None or distance_error <= epsilon))
        results.append(dict(
            projection=projection, direction=direction, passed=passed,
            orientation_error=orientation_error, cplane_origin_error=plane_error,
            cplane_axes_error=axes_error,
            target_error=target_error, perspective_distance=distance if projection == "Perspective" else None,
            perspective_distance_error=distance_error,
            projection_matches=projection_matches,
        ))
    return dict(passed=all(row["passed"] for row in results), views=results)


if __name__ == "__main__":
    import json
    import sys
    if len(sys.argv) != 3:
        raise SystemExit("usage: python3 -m tools.rhino_oracle.view_camera_probe FIXTURE RHINO_RESPONSE")
    with open(sys.argv[1]) as stream:
        request = json.load(stream)
    with open(sys.argv[2]) as stream:
        response = json.load(stream)
    operation = request["operations"][0]
    rows = response["results"][0]["value"]
    result = compare_to_viboceros(operation, rows)
    print(json.dumps(result, indent=2, sort_keys=True))
    if not result["passed"]:
        raise SystemExit(1)
