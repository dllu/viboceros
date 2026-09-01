"""Standalone RhinoPython worker for the versioned geometry oracle protocol.

This file is copied beside request.json and executed inside Rhino. Keep its
syntax compatible with both Rhino 8 Python 3 and the legacy IronPython host.
"""

import json
import math
import os
from timeit import default_timer

import Rhino


PROTOCOL_VERSION = 1
MAX_ITERATIONS = 1000000
DEFAULT_TOLERANCE = {
    "absolute": 1.0e-9,
    "relative": 1.0e-12,
    "angular": 1.0e-10,
}
try:
    string_types = (basestring,)
except NameError:
    string_types = (str,)
try:
    iteration_range = xrange
except NameError:
    iteration_range = range


def _point(coordinates):
    values = [_finite(value, "point coordinate") for value in coordinates]
    if len(values) != 3:
        raise ValueError("point must contain exactly three coordinates")
    return Rhino.Geometry.Point3d(values[0], values[1], values[2])


def _vector(coordinates):
    values = [_finite(value, "vector coordinate") for value in coordinates]
    if len(values) != 3:
        raise ValueError("vector must contain exactly three coordinates")
    return Rhino.Geometry.Vector3d(values[0], values[1], values[2])


def _xyz(value):
    return [float(value.X), float(value.Y), float(value.Z)]


def _finite(value, context):
    number = float(value)
    if math.isnan(number) or math.isinf(number):
        raise ValueError("%s must be finite" % context)
    return number


def _unit(vector, tolerance, context):
    length = float(vector.Length)
    if not length > tolerance:
        raise ValueError("%s is degenerate" % context)
    vector /= length
    return vector


def _measure(iterations, operation):
    value = operation()
    started = default_timer()
    for _unused in iteration_range(iterations):
        value = operation()
    elapsed_ns = int(round((default_timer() - started) * 1000000000.0))
    return value, max(0, elapsed_ns)


def _canonical_join_segments(curves):
    polylines = []
    for curve in curves:
        segments = curve.DuplicateSegments()
        if segments is None or len(segments) == 0:
            values = [[_xyz(curve.PointAtStart), _xyz(curve.PointAtEnd)]]
        else:
            values = [
                [_xyz(segment.PointAtStart), _xyz(segment.PointAtEnd)]
                for segment in segments
            ]
        if tuple(values[-1][1]) < tuple(values[0][0]):
            values.reverse()
            values = [[segment[1], segment[0]] for segment in values]
        polylines.append(values)
    polylines.sort(
        key=lambda segments: tuple(
            tuple(point) for segment in segments for point in segment
        )
    )
    return polylines


def _set_knots(target, full_knots, context):
    values = [_finite(value, context) for value in full_knots]
    if len(values) != target.Count + 2:
        raise ValueError(
            "%s count must be Rhino knot count plus two" % context
        )
    for index, value in enumerate(values[1:-1]):
        target[index] = value


def _set_curve_controls(curve, controls):
    if len(controls) != curve.Points.Count:
        raise ValueError("NURBS curve control-point count does not match")
    for index, control in enumerate(controls):
        point = _point(control["point"])
        weight = _finite(control.get("weight", 1.0), "control-point weight")
        if not weight > 0.0 or not curve.Points.SetPoint(index, point, weight):
            raise ValueError("invalid NURBS curve control point")


def _set_surface_controls(surface, controls, count_u, count_v):
    if len(controls) != count_u * count_v:
        raise ValueError("NURBS surface control-net size does not match")
    for v_index in range(count_v):
        for u_index in range(count_u):
            control = controls[v_index * count_u + u_index]
            point = _point(control["point"])
            weight = _finite(control.get("weight", 1.0), "control-point weight")
            if not weight > 0.0 or not surface.Points.SetPoint(
                u_index, v_index, point, weight
            ):
                raise ValueError("invalid NURBS surface control point")


def _execute(operation, iterations, tolerance):
    kind = operation["op"]
    if kind == "point_distance":
        a = _point(operation["a"])
        b = _point(operation["b"])
        value, elapsed = _measure(iterations, lambda: a.DistanceTo(b))
        return float(value), elapsed

    if kind == "line_point":
        line = Rhino.Geometry.Line(
            _point(operation["start"]), _point(operation["end"])
        )
        if not line.IsValid:
            raise ValueError("line is degenerate")
        parameter = _finite(operation["parameter"], "line parameter")
        value, elapsed = _measure(iterations, lambda: line.PointAt(parameter))
        return _xyz(value), elapsed

    if kind == "circle_point":
        center = _point(operation["center"])
        normal = _unit(
            _vector(operation["normal"]), tolerance["absolute"], "circle normal"
        )
        x_axis = _unit(
            _vector(operation["x_axis"]), tolerance["absolute"], "circle x axis"
        )
        projection = x_axis - normal * (x_axis * normal)
        projected_length = float(projection.Length)
        x_axis = _unit(projection, tolerance["angular"], "circle x axis")
        y_axis = Rhino.Geometry.Vector3d.CrossProduct(normal, x_axis)
        radius = _finite(operation["radius"], "circle radius") * projected_length
        if not radius > tolerance["absolute"]:
            raise ValueError("circle radius is degenerate")
        plane = Rhino.Geometry.Plane(center, x_axis, y_axis)
        circle = Rhino.Geometry.Circle(plane, radius)
        angle = _finite(operation["angle_radians"], "circle angle")
        value, elapsed = _measure(iterations, lambda: circle.PointAt(angle))
        return _xyz(value), elapsed

    if kind == "arc_three_point":
        arc = Rhino.Geometry.Arc(
            _point(operation["start"]),
            _point(operation["through"]),
            _point(operation["end"]),
        )
        if not arc.IsValid:
            raise ValueError("three-point arc is degenerate")
        normalized = _finite(
            operation["normalized_parameter"], "normalized arc parameter"
        )
        if normalized < 0.0 or normalized > 1.0:
            raise ValueError("normalized arc parameter is outside [0, 1]")
        parameter = arc.AngleDomain.ParameterAt(normalized)
        value, elapsed = _measure(iterations, lambda: arc.PointAt(parameter))
        return {
            "center": _xyz(arc.Center),
            "point": _xyz(value),
            "radius": float(arc.Radius),
            "sweep_radians": float(arc.Angle),
        }, elapsed

    if kind == "ellipse_three_point":
        ellipse = Rhino.Geometry.Ellipse(
            _point(operation["center"]),
            _point(operation["first_axis_point"]),
            _point(operation["second_axis_point"]),
        )
        if not ellipse.IsValid:
            raise ValueError("three-point ellipse is degenerate")
        angle = _finite(operation["angle_radians"], "ellipse angle")
        plane = ellipse.Plane

        def ellipse_point():
            return plane.PointAt(
                ellipse.Radius1 * math.cos(angle),
                ellipse.Radius2 * math.sin(angle),
            )

        value, elapsed = _measure(iterations, ellipse_point)
        return {
            "center": _xyz(plane.Origin),
            "point": _xyz(value),
            "radius_x": float(ellipse.Radius1),
            "radius_y": float(ellipse.Radius2),
            "x_axis": _xyz(plane.XAxis),
            "y_axis": _xyz(plane.YAxis),
        }, elapsed

    if kind == "polyline_length":
        polyline = Rhino.Geometry.Polyline(
            [_point(vertex) for vertex in operation["vertices"]]
        )
        if not polyline.IsValid:
            raise ValueError("polyline is invalid")
        value, elapsed = _measure(iterations, lambda: polyline.Length)
        return float(value), elapsed

    if kind == "polyline_join":
        curves = []
        for vertices in operation["polylines"]:
            polyline = Rhino.Geometry.Polyline(
                [_point(vertex) for vertex in vertices]
            )
            if not polyline.IsValid:
                raise ValueError("polyline to join is invalid")
            curves.append(Rhino.Geometry.PolylineCurve(polyline))

        def join_curves():
            return Rhino.Geometry.Curve.JoinCurves(
                curves, tolerance["absolute"], False
            )

        value, elapsed = _measure(iterations, join_curves)
        return _canonical_join_segments(value), elapsed

    if kind == "nurbs_curve_evaluate":
        degree = int(operation["degree"])
        controls = operation["control_points"]
        curve = Rhino.Geometry.NurbsCurve(3, True, degree + 1, len(controls))
        _set_curve_controls(curve, controls)
        _set_knots(curve.Knots, operation["knots"], "curve knot")
        if not curve.IsValid:
            raise ValueError("NURBS curve is invalid")
        parameter = _finite(operation["parameter"], "curve parameter")
        value, elapsed = _measure(
            iterations, lambda: curve.DerivativeAt(parameter, 1)
        )
        if value is None or len(value) < 2:
            raise ValueError("NURBS curve evaluation failed")
        return {"point": _xyz(value[0]), "derivative": _xyz(value[1])}, elapsed

    if kind == "nurbs_surface_evaluate":
        degree_u = int(operation["degree_u"])
        degree_v = int(operation["degree_v"])
        count_u = int(operation["control_point_count_u"])
        count_v = int(operation["control_point_count_v"])
        surface = Rhino.Geometry.NurbsSurface.Create(
            3, True, degree_u + 1, degree_v + 1, count_u, count_v
        )
        if surface is None:
            raise ValueError("could not allocate NURBS surface")
        _set_surface_controls(
            surface, operation["control_points"], count_u, count_v
        )
        _set_knots(surface.KnotsU, operation["knots_u"], "surface U knot")
        _set_knots(surface.KnotsV, operation["knots_v"], "surface V knot")
        if not surface.IsValid:
            raise ValueError("NURBS surface is invalid")
        u_value = _finite(operation["u"], "surface U parameter")
        v_value = _finite(operation["v"], "surface V parameter")
        value, elapsed = _measure(
            iterations, lambda: surface.Evaluate(u_value, v_value, 1)
        )
        if value is None or len(value) < 3 or not value[0]:
            raise ValueError("NURBS surface evaluation failed")
        point = value[1]
        derivatives = value[2]
        if derivatives is None or len(derivatives) < 2:
            raise ValueError("NURBS surface derivatives are missing")
        derivative_u = derivatives[0]
        derivative_v = derivatives[1]
        normal = Rhino.Geometry.Vector3d.CrossProduct(derivative_u, derivative_v)
        normal = _unit(normal, tolerance["absolute"], "surface normal")
        return {
            "point": _xyz(point),
            "derivative_u": _xyz(derivative_u),
            "derivative_v": _xyz(derivative_v),
            "normal": _xyz(normal),
        }, elapsed

    raise ValueError("unsupported oracle operation: %s" % kind)


def _validate_request(request):
    if request.get("protocol_version") != PROTOCOL_VERSION:
        raise ValueError(
            "unsupported oracle protocol version %r" % request.get("protocol_version")
        )
    iterations = request.get("iterations", 1)
    if isinstance(iterations, bool) or int(iterations) != iterations:
        raise ValueError("oracle iterations must be an integer")
    iterations = int(iterations)
    if iterations < 1 or iterations > MAX_ITERATIONS:
        raise ValueError("oracle iterations must be from 1 through %d" % MAX_ITERATIONS)
    operations = request.get("operations")
    if not isinstance(operations, list):
        raise ValueError("oracle operations must be an array")
    ids = set()
    for operation in operations:
        operation_id = operation.get("id")
        if not isinstance(operation_id, string_types) or not operation_id.strip():
            raise ValueError("oracle operation id must be a non-empty string")
        if operation_id in ids:
            raise ValueError("duplicated oracle operation id: %s" % operation_id)
        ids.add(operation_id)
    tolerance = dict(DEFAULT_TOLERANCE)
    tolerance.update(request.get("tolerance") or {})
    for name in ("absolute", "relative", "angular"):
        tolerance[name] = _finite(tolerance[name], "%s tolerance" % name)
        if not tolerance[name] > 0.0:
            raise ValueError("%s tolerance must be positive" % name)
    return iterations, operations, tolerance


def _response(request):
    iterations = request.get("iterations", 1) if isinstance(request, dict) else 1
    response = {
        "protocol_version": PROTOCOL_VERSION,
        "engine": "rhino",
        "engine_version": str(Rhino.RhinoApp.Version),
        "iterations": iterations,
        "results": [],
    }
    try:
        iterations, operations, tolerance = _validate_request(request)
        response["iterations"] = iterations
        for operation in operations:
            value, elapsed = _execute(operation, iterations, tolerance)
            response["results"].append(
                {
                    "id": operation["id"],
                    "value": value,
                    "elapsed_ns": elapsed,
                }
            )
    except Exception as error:
        response["results"] = []
        response["error"] = "%s: %s" % (type(error).__name__, error)
    return response


def _main():
    job_directory = os.path.dirname(os.path.abspath(__file__))
    request_path = os.path.join(job_directory, "request.json")
    response_path = os.path.join(job_directory, "response.json")
    temporary_path = response_path + ".tmp"
    request = {}
    try:
        with open(request_path, "r") as stream:
            request = json.load(stream)
        response = _response(request)
    except Exception as error:
        response = {
            "protocol_version": PROTOCOL_VERSION,
            "engine": "rhino",
            "engine_version": str(Rhino.RhinoApp.Version),
            "iterations": 1,
            "results": [],
            "error": "%s: %s" % (type(error).__name__, error),
        }
    with open(temporary_path, "w") as stream:
        json.dump(response, stream, indent=2, allow_nan=False)
        stream.write("\n")
    if os.path.exists(response_path):
        os.remove(response_path)
    os.rename(temporary_path, response_path)
    host_options = {}
    if isinstance(request, dict):
        host_options = request.get("_host") or {}
    if host_options.get("exit_rhino_when_complete"):
        Rhino.RhinoApp.Exit(False)


_main()
