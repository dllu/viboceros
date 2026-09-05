# -*- coding: utf-8 -*-
"""Standalone RhinoPython worker for the versioned compatibility oracle.

This file is copied beside request.json and executed inside Rhino. Keep its
syntax compatible with both Rhino 8 Python 3 and the legacy IronPython host.
"""

import json
import math
import os
from contextlib import contextmanager
from timeit import default_timer

import Rhino
import System


PROTOCOL_VERSION = 1
MAX_ITERATIONS = 1000000
MAX_STATE_CYCLE_OBJECTS = 100000
DEFAULT_TOLERANCE = {
    "absolute": 1.0e-9,
    "relative": 1.0e-12,
    "angular": 1.0e-10,
}
LAST_PROGRESS_STAGE = "worker loading"
try:
    string_types = (basestring,)
except NameError:
    string_types = (str,)
try:
    iteration_range = xrange
except NameError:
    iteration_range = range


def _record_progress(stage):
    global LAST_PROGRESS_STAGE
    LAST_PROGRESS_STAGE = stage
    path = os.path.join(
        os.path.dirname(os.path.abspath(__file__)), "worker-progress.log"
    )
    try:
        with open(path, "a") as stream:
            stream.write(stage + "\n")
            stream.flush()
    except Exception:
        pass


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


def _command_point(coordinates):
    point = _point(coordinates)
    return "%.17g,%.17g,%.17g" % (point.X, point.Y, point.Z)


def _xy(value):
    return [float(value.X), float(value.Y)]


def _finite(value, context):
    number = float(value)
    if math.isnan(number) or math.isinf(number):
        raise ValueError("%s must be finite" % context)
    return number


def _wildcard_matches(pattern, candidate):
    pattern = pattern.lower()
    candidate = candidate.lower()
    pattern_index = 0
    candidate_index = 0
    star_index = None
    star_candidate_index = 0
    while candidate_index < len(candidate):
        if pattern_index < len(pattern) and (
            pattern[pattern_index] == "?"
            or pattern[pattern_index] == candidate[candidate_index]
        ):
            pattern_index += 1
            candidate_index += 1
        elif pattern_index < len(pattern) and pattern[pattern_index] == "*":
            star_index = pattern_index
            pattern_index += 1
            star_candidate_index = candidate_index
        elif star_index is not None:
            pattern_index = star_index + 1
            star_candidate_index += 1
            candidate_index = star_candidate_index
        else:
            return False
    while pattern_index < len(pattern) and pattern[pattern_index] == "*":
        pattern_index += 1
    return pattern_index == len(pattern)


def _state_cycle_indices(operation, name, object_count):
    values = operation.get(name)
    if not isinstance(values, list):
        raise ValueError("%s must be an array" % name)
    indices = set()
    for value in values:
        if isinstance(value, bool) or int(value) != value:
            raise ValueError("%s must contain integer indices" % name)
        index = int(value)
        if index < 0 or index >= object_count:
            raise ValueError(
                "%s index %d is outside object count %d"
                % (name, index, object_count)
            )
        indices.add(index)
    return sorted(indices)


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


def _polygon_mesh(vertices, faces):
    mesh = Rhino.Geometry.Mesh()
    try:
        for vertex in vertices:
            if mesh.Vertices.Add(_point(vertex)) < 0:
                raise ValueError("could not add mesh vertex")
        for face in faces:
            if len(face) not in (3, 4):
                raise ValueError("mesh face must contain three or four indices")
            if any(
                isinstance(index, bool) or int(index) != index for index in face
            ):
                raise ValueError("mesh face index must be an integer")
            indices = [int(index) for index in face]
            added = (
                mesh.Faces.AddFace(indices[0], indices[1], indices[2])
                if len(indices) == 3
                else mesh.Faces.AddFace(
                    indices[0], indices[1], indices[2], indices[3]
                )
            )
            if added < 0:
                raise ValueError("could not add mesh face")
        if not mesh.IsValid:
            raise ValueError("mesh is invalid")
        return mesh
    except Exception:
        mesh.Dispose()
        raise


def _triangle_mesh(vertices, triangles):
    if any(len(triangle) != 3 for triangle in triangles):
        raise ValueError("triangle mesh face must contain exactly three indices")
    return _polygon_mesh(vertices, triangles)


def _mesh_triangles(mesh):
    triangles = []
    for index in range(mesh.Faces.Count):
        face = mesh.Faces[index]
        if not face.IsTriangle:
            raise ValueError("oracle mesh unexpectedly contains a quad")
        triangles.append([int(face.A), int(face.B), int(face.C)])
    return triangles


def _mesh_value(mesh):
    return {
        "triangles": _mesh_triangles(mesh),
        "vertices": [
            _xyz(mesh.Vertices.Point3dAt(index))
            for index in range(mesh.Vertices.Count)
        ],
    }


def _polygon_mesh_value(mesh):
    faces = []
    for index in range(mesh.Faces.Count):
        face = mesh.Faces[index]
        indices = [int(face.A), int(face.B), int(face.C)]
        if face.IsQuad:
            indices.append(int(face.D))
        faces.append(indices)
    return {
        "faces": faces,
        "vertices": [
            _xyz(mesh.Vertices.Point3dAt(index))
            for index in range(mesh.Vertices.Count)
        ],
    }


def _canonical_polygon_mesh_face_value(mesh):
    faces = []
    triangle_count = 0
    quad_count = 0
    for index in range(mesh.Faces.Count):
        face = mesh.Faces[index]
        indices = [int(face.A), int(face.B), int(face.C)]
        if face.IsQuad:
            indices.append(int(face.D))
            quad_count += 1
        else:
            triangle_count += 1
        points = [
            tuple(_xyz(mesh.Vertices.Point3dAt(vertex))) for vertex in indices
        ]
        rotations = [tuple(points[offset:] + points[:offset]) for offset in range(len(points))]
        faces.append(min(rotations))
    faces.sort()
    return {
        "faces": faces,
        "quad_count": quad_count,
        "triangle_count": triangle_count,
    }


def _mesh_to_nurb_brep_value(brep):
    faces = []
    for face in brep.Faces:
        surface = face.UnderlyingSurface()
        domain_u = surface.Domain(0)
        domain_v = surface.Domain(1)
        loops = []
        for loop in face.Loops:
            trims = []
            for trim in loop.Trims:
                trims.append(
                    {
                        "edge": None if trim.Edge is None else int(trim.Edge.EdgeIndex),
                        "end": _xy(trim.PointAtEnd),
                        "iso": str(trim.IsoStatus),
                        "reversed": bool(trim.IsReversed()),
                        "start": _xy(trim.PointAtStart),
                        "type": str(trim.TrimType),
                    }
                )
            loops.append({"trims": trims, "type": str(loop.LoopType)})
        faces.append(
            {
                "corners": [
                    _xyz(surface.PointAt(domain_u.T0, domain_v.T0)),
                    _xyz(surface.PointAt(domain_u.T1, domain_v.T0)),
                    _xyz(surface.PointAt(domain_u.T1, domain_v.T1)),
                    _xyz(surface.PointAt(domain_u.T0, domain_v.T1)),
                ],
                "degree": [int(surface.Degree(0)), int(surface.Degree(1))],
                "loops": loops,
                "reversed": bool(face.OrientationIsReversed),
            }
        )
    return {
        "edge_count": int(brep.Edges.Count),
        "edges": [
            {
                "domain": [float(edge.Domain.T0), float(edge.Domain.T1)],
                "vertices": [
                    int(edge.StartVertex.VertexIndex),
                    int(edge.EndVertex.VertexIndex),
                ],
            }
            for edge in brep.Edges
        ],
        "faces": faces,
        "is_solid": bool(brep.IsSolid),
        "vertex_count": int(brep.Vertices.Count),
        "vertices": [_xyz(vertex.Location) for vertex in brep.Vertices],
    }


def _nurbs_curve_definition(curve, canonicalize_parameters=False):
    nurbs = curve.ToNurbsCurve()
    if nurbs is None:
        raise ValueError("could not convert curve to NURBS form")
    try:
        knots = [float(nurbs.Knots[0])]
        knots.extend(float(nurbs.Knots[index]) for index in range(nurbs.Knots.Count))
        knots.append(float(nurbs.Knots[nurbs.Knots.Count - 1]))
        domain = [float(nurbs.Domain.T0), float(nurbs.Domain.T1)]
        if canonicalize_parameters:
            rank = 0
            ranks = []
            previous = None
            for knot in knots:
                if previous is not None and knot != previous:
                    rank += 1
                ranks.append(rank)
                previous = knot
            final_rank = float(max(rank, 1))
            knots = [value / final_rank for value in ranks]
            domain = [0.0, 1.0]
        return {
            "control_points": [
                {
                    "point": _xyz(nurbs.Points[index].Location),
                    "weight": float(nurbs.Points[index].Weight),
                }
                for index in range(nurbs.Points.Count)
            ],
            "degree": int(nurbs.Degree),
            "domain": domain,
            "knots": knots,
        }
    finally:
        nurbs.Dispose()


def _nurbs_parameter_curve_definition(curve):
    definition = _nurbs_curve_definition(curve)
    for control in definition["control_points"]:
        control["point"] = control["point"][:2]
    return definition


def _surface_split_trim_value(curve, surface, sample_geometry):
    if not sample_geometry:
        return _nurbs_parameter_curve_definition(curve)
    points = [curve.PointAtStart]
    for index in range(1, 64):
        success, parameter = curve.NormalizedLengthParameter(index / 64.0, 1e-12)
        if not success:
            raise ValueError("could not sample the split trim at equal UV arc lengths")
        points.append(curve.PointAt(parameter))
    points.append(curve.PointAtEnd)
    return {
        "domain": [float(curve.Domain.T0), float(curve.Domain.T1)],
        "uv_points": [_xy(point) for point in points],
        "surface_points": [_xyz(surface.PointAt(point.X, point.Y)) for point in points],
    }


def _polycurve_document_record(curve):
    document = Rhino.RhinoDoc.ActiveDoc

    def object_ids():
        settings = Rhino.DocObjects.ObjectEnumeratorSettings()
        settings.NormalObjects = True
        settings.LockedObjects = True
        settings.HiddenObjects = True
        return set(obj.Id for obj in document.Objects.GetObjectList(settings))

    def outputs(command, describe):
        document.Objects.UnselectAll()
        before = object_ids()
        source_id = document.Objects.AddCurve(curve)
        if source_id == System.Guid.Empty:
            raise ValueError("could not add polycurve command source")
        try:
            script = "_-%s _SelID %s _Enter" % (command, source_id)
            if not Rhino.RhinoApp.RunScript(script, False):
                raise ValueError("polycurve command failed: " + command)
            return [describe(document.Objects.FindId(object_id).Geometry)
                    for object_id in object_ids() - before - set([source_id])]
        finally:
            document.Objects.UnselectAll()
            for object_id in object_ids() - before:
                document.Objects.Delete(object_id, True)

    points = outputs("ExtractPt _Output=Points _OutputLayer=Current", lambda geometry: _xyz(geometry.Location))

    def polygon(geometry):
        success, polyline = geometry.TryGetPolyline()
        if not success:
            raise ValueError("control polygon output is not a polyline")
        return [_xyz(point) for point in polyline]

    polygons = outputs("ExtractControlPolygon _OutputLayer=Current", polygon)
    exploded = outputs("Explode", _nurbs_curve_definition)
    exploded.sort(key=lambda item: item["domain"][0])
    model = Rhino.FileIO.File3dm()
    decoded = None
    path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "polycurve-%s.3dm" % System.Guid.NewGuid())
    reversed_curve = curve.DuplicateCurve()
    reparameterized = curve.DuplicateCurve()
    try:
        if model.Layers.AddDefaultLayer("Default", System.Drawing.Color.Black) < 0:
            raise ValueError("could not add polycurve 3DM layer")
        model.Objects.AddCurve(curve)
        if not model.Write(path, 8):
            raise ValueError("could not write polycurve 3DM")
        decoded = Rhino.FileIO.File3dm.Read(path)
        if decoded is None:
            raise ValueError("could not read polycurve 3DM")
        objects = list(decoded.Objects)
        if len(objects) != 1 or not isinstance(objects[0].Geometry, Rhino.Geometry.PolyCurve):
            raise ValueError("3DM polycurve type was lost")
        result = objects[0].Geometry
        segments = []
        for index in range(result.SegmentCount):
            segment = result.SegmentCurve(index).DuplicateCurve()
            try:
                segment.Domain = result.SegmentDomain(index)
                segments.append(_nurbs_curve_definition(segment))
            finally:
                segment.Dispose()
        if not reversed_curve.Reverse():
            raise ValueError("could not reverse polycurve")
        reparameterized.Domain = Rhino.Geometry.Interval(0.0, 1.0)
        return {"extract_points": sorted(points, key=lambda p: tuple(round(v * 1e9) for v in p)), "control_polygons": sorted(polygons),
                "exploded": exploded, "round_trip_segments": segments,
                "reversed_duplicate": bool(Rhino.Geometry.GeometryBase.GeometryEquals(curve, reversed_curve)),
                "reparameterized_duplicate": bool(Rhino.Geometry.GeometryBase.GeometryEquals(curve, reparameterized))}
    finally:
        reversed_curve.Dispose()
        reparameterized.Dispose()
        if decoded is not None:
            decoded.Dispose()
        model.Dispose()
        if os.path.exists(path):
            os.remove(path)


def _polycurve_geometry(operation, iterations, tolerance):
    source = Rhino.Geometry.PolyCurve()
    try:
        for definition in operation["segments"]:
            segment = _nurbs_curve_from_definition(definition)
            try:
                if not source.AppendSegment(segment):
                    raise ValueError("could not append exact polycurve segment")
            finally:
                segment.Dispose()
        if not source.IsValid:
            raise ValueError("invalid polycurve fixture")

        def record(curve):
            if operation["op"] == "polycurve_document":
                return _polycurve_document_record(curve)
            count = curve.SegmentCount if isinstance(curve, Rhino.Geometry.PolyCurve) else 1
            domains = [curve.SegmentDomain(i) for i in range(count)] if isinstance(curve, Rhino.Geometry.PolyCurve) else [curve.Domain]
            parameters = [float(curve.Domain.ParameterAt(float(i) / 32.0)) for i in range(33)]
            parameters.extend(float(domain.T0) for domain in domains)
            parameters.append(float(curve.Domain.T1))
            samples = []
            for parameter in sorted(set(parameters)):
                derivatives = curve.DerivativeAt(parameter, 2)
                if derivatives is None or len(derivatives) != 3:
                    raise ValueError("could not evaluate polycurve derivatives")
                samples.append({"parameter": parameter, "point": _xyz(curve.PointAt(parameter)),
                                "first": _xyz(derivatives[1]), "second": _xyz(derivatives[2])})
            segments = []
            for i, domain in enumerate(domains):
                segment = curve.SegmentCurve(i).DuplicateCurve() if isinstance(curve, Rhino.Geometry.PolyCurve) else curve.DuplicateCurve()
                try:
                    segment.Domain = domain
                    segments.append(_nurbs_curve_definition(segment))
                finally:
                    segment.Dispose()
            divisions = curve.DivideByCount(17, True)
            expected_count = 17 if curve.IsClosed else 18
            if divisions is None or len(divisions) != expected_count:
                raise ValueError("polycurve division returned an unexpected point count")
            division_points = []
            exact_nurbs = curve.ToNurbsCurve()
            if exact_nurbs is None:
                raise ValueError("could not convert polycurve for length inversion")
            try:
                for index in range(expected_count):
                    # Check public division topology above. Use the exact NURBS
                    # form for tolerance-bearing length inversion: the composite
                    # API retains coarse internal segment-length inversions.
                    success, parameter = exact_nurbs.NormalizedLengthParameter(float(index) / 17.0, tolerance["relative"] * 0.001)
                    if not success:
                        raise ValueError("could not divide polycurve at requested tolerance")
                    division_points.append(_xyz(exact_nurbs.PointAt(parameter)))
            finally:
                exact_nurbs.Dispose()
            return {"domain": [float(curve.Domain.T0), float(curve.Domain.T1)],
                    "segment_domains": [[float(d.T0), float(d.T1)] for d in domains],
                    "segments": segments, "samples": samples, "closed": bool(curve.IsClosed),
                    "length": float(curve.GetLength(tolerance["relative"])),
                    "division_points": division_points,
                    "division_count_without_ends": len(curve.DivideByCount(17, False))}

        def compute():
            owned = [source.DuplicateCurve()]
            try:
                curve = owned[0]
                if operation.get("domain") is not None:
                    curve.Domain = Rhino.Geometry.Interval(*operation["domain"])
                if operation.get("reversed", False) and not curve.Reverse():
                    raise ValueError("could not reverse polycurve")
                if operation.get("trim") is not None:
                    curve = curve.Trim(Rhino.Geometry.Interval(*operation["trim"]))
                    if curve is None:
                        raise ValueError("could not trim polycurve")
                    owned.append(curve)
                if operation.get("split") is not None:
                    curves = curve.Split(float(operation["split"]))
                    if curves is None or len(curves) != 2:
                        raise ValueError("could not split polycurve")
                    owned.extend(curves)
                else:
                    curves = [curve]
                if operation["op"] == "polycurve_document":
                    return record(curve)
                return {"curves": [record(c) for c in curves]}
            finally:
                for curve in reversed(owned):
                    curve.Dispose()
        return _measure(iterations, compute)
    finally:
        source.Dispose()


def _trimmed_surface_mass_properties(operation, iterations, tolerance):
    owned = []
    try:
        brep = Rhino.Geometry.Brep()
        owned.append(brep)
        capped = operation.get("cap_surface") is not None
        parameter_indices = []
        for index, boundary in enumerate(operation["boundaries"]):
            spatial = _nurbs_curve_from_definition(boundary["curve"])
            owned.append(spatial)
            parameter = _nurbs_curve_from_definition(boundary["parameter_curve"], 2)
            owned.append(parameter)
            if not spatial.IsClosed or not parameter.IsClosed:
                raise ValueError("mass property boundaries must be closed")
            brep.Vertices.Add(spatial.PointAtStart, 0.0)
            curve_index = brep.Curves3D.Add(spatial)
            brep.Edges.Add(index, index, curve_index, 0.0)
            parameter_indices.append(brep.Curves2D.Add(parameter))

        def add_face(definition, reversed_face):
            surface = _nurbs_surface_from_definition(definition)
            owned.append(surface)
            face = brep.Faces.Add(brep.AddSurface(surface))
            face.OrientationIsReversed = reversed_face
            for index, parameter_index in enumerate(parameter_indices):
                loop_type = Rhino.Geometry.BrepLoopType.Outer if index == 0 else Rhino.Geometry.BrepLoopType.Inner
                loop = brep.Loops.Add(loop_type, face)
                trim = brep.Trims.Add(brep.Edges[index], False, loop, parameter_index)
                trim.TrimType = Rhino.Geometry.BrepTrimType.Mated if capped else Rhino.Geometry.BrepTrimType.Boundary
                trim.IsoStatus = getattr(Rhino.Geometry.IsoStatus, "None")
                trim.SetTolerances(0.0, 0.0)

        reversed_face = bool(operation.get("reversed", False))
        add_face(operation["surface"], reversed_face)
        cap = operation.get("cap_surface")
        if cap is not None:
            add_face(cap, not reversed_face)
        valid, log = brep.IsValidWithLog()
        if not valid:
            raise ValueError("invalid mass property B-rep: %s" % log)
        if capped and not brep.IsSolid:
            raise ValueError("capped mass property fixture is not solid")
        u, v = operation["interior_uv"]
        if str(brep.Faces[0].IsPointOnFace(u, v)) != "Interior":
            raise ValueError("mass property interior point must lie in the retained face")

        def compute():
            properties = Rhino.Geometry.AreaMassProperties.Compute(
                brep, True, False, False, False, tolerance["relative"], tolerance["absolute"]
            )
            if properties is None:
                raise ValueError("trimmed surface area integration failed")
            try:
                area = float(properties.Area)
            finally:
                properties.Dispose()
            volume = None
            if brep.IsSolid:
                properties = Rhino.Geometry.VolumeMassProperties.Compute(
                    brep, True, False, False, False, tolerance["relative"], tolerance["absolute"]
                )
                if properties is None:
                    raise ValueError("trimmed surface volume integration failed")
                try:
                    volume = float(properties.Volume)
                finally:
                    properties.Dispose()
            return {"area": area, "volume": volume, "is_solid": bool(brep.IsSolid)}
        return _measure(iterations, compute)
    finally:
        for geometry in reversed(owned):
            geometry.Dispose()


def _canonical_closed_intersection_curve_definition(curve):
    definition = _nurbs_curve_definition(curve)
    controls = definition["control_points"]
    if definition["degree"] != 1 or len(controls) < 3:
        return definition
    first = controls[0]["point"]
    last = controls[-1]["point"]
    if sum((first[index] - last[index]) ** 2 for index in range(3)) > 1e-18:
        return definition
    unique = controls[:-1]
    seam = min(
        range(len(unique)),
        key=lambda index: tuple(
            round(value, 9) for value in unique[index]["point"]
        ),
    )
    rotated = unique[seam:] + unique[:seam]
    rotated.append(rotated[0])
    segment_count = len(rotated) - 1
    definition["control_points"] = rotated
    definition["domain"] = [0.0, 1.0]
    definition["knots"] = (
        [0.0, 0.0]
        + [float(index) / float(segment_count) for index in range(1, segment_count)]
        + [1.0, 1.0]
    )
    return definition


def _canonical_linear_intersection_curve_definition(curve):
    definition = _nurbs_curve_definition(curve)
    controls = definition["control_points"]
    if definition["degree"] != 1 or len(controls) < 2:
        return definition
    if any(abs(control["weight"] - controls[0]["weight"]) > 1e-12 for control in controls):
        return definition
    quantized_point = lambda control: tuple(
        round(value, 9) for value in control["point"]
    )
    first = controls[0]["point"]
    last = controls[-1]["point"]
    closed = sum((first[index] - last[index]) ** 2 for index in range(3)) <= 1e-18
    if closed:
        unique = controls[:-1]
        seam = min(range(len(unique)), key=lambda index: quantized_point(unique[index]))
        forward = unique[seam:] + unique[:seam]
        reverse = [forward[0]] + list(reversed(forward[1:]))
        if tuple(map(quantized_point, reverse)) < tuple(map(quantized_point, forward)):
            forward = reverse
        controls = forward + [forward[0]]
    elif quantized_point(controls[-1]) < quantized_point(controls[0]):
        controls = list(reversed(controls))
    segment_count = len(controls) - 1
    definition["control_points"] = controls
    definition["domain"] = [0.0, 1.0]
    definition["knots"] = (
        [0.0, 0.0]
        + [float(index) / float(segment_count) for index in range(1, segment_count)]
        + [1.0, 1.0]
    )
    return definition


def _nurbs_curve_from_definition(definition, dimension=3):
    degree = int(definition["degree"])
    controls = definition["control_points"]
    curve = Rhino.Geometry.NurbsCurve(dimension, True, degree + 1, len(controls))
    try:
        _set_curve_controls(curve, controls)
        _set_knots(curve.Knots, definition["knots"], "curve knot")
        domain = definition.get("domain")
        if domain is not None:
            curve.Domain = Rhino.Geometry.Interval(
                _finite(domain[0], "curve domain"),
                _finite(domain[1], "curve domain"),
            )
        if not curve.IsValid:
            raise ValueError("NURBS curve definition is invalid")
        return curve
    except Exception:
        curve.Dispose()
        raise


def _nurbs_surface_from_definition(definition):
    degree_u = int(definition["degree_u"])
    degree_v = int(definition["degree_v"])
    count_u = int(definition["control_point_count_u"])
    count_v = int(definition["control_point_count_v"])
    surface = Rhino.Geometry.NurbsSurface.Create(
        3, True, degree_u + 1, degree_v + 1, count_u, count_v
    )
    if surface is None:
        raise ValueError("could not allocate NURBS surface")
    try:
        _set_surface_controls(surface, definition["control_points"], count_u, count_v)
        _set_knots(surface.KnotsU, definition["knots_u"], "surface U knot")
        _set_knots(surface.KnotsV, definition["knots_v"], "surface V knot")
        domain_u = definition.get("domain_u")
        domain_v = definition.get("domain_v")
        if (domain_u is None) != (domain_v is None):
            raise ValueError("surface boundary requires both domains or neither")
        if domain_u is not None:
            set_u = surface.SetDomain(
                0,
                Rhino.Geometry.Interval(
                    _finite(domain_u[0], "surface U domain"),
                    _finite(domain_u[1], "surface U domain"),
                ),
            )
            set_v = surface.SetDomain(
                1,
                Rhino.Geometry.Interval(
                    _finite(domain_v[0], "surface V domain"),
                    _finite(domain_v[1], "surface V domain"),
                ),
            )
            if not set_u or not set_v:
                raise ValueError("surface boundary domains are invalid")
        if not surface.IsValid:
            raise ValueError("NURBS surface boundary is invalid")
        return surface
    except Exception:
        surface.Dispose()
        raise


def _curve_extension_boundary_from_definition(definition, tolerance):
    surface_definition = definition.get("surface")
    if surface_definition is not None:
        return _nurbs_surface_from_definition(surface_definition)
    planar_face_definition = definition.get("planar_face")
    if planar_face_definition is not None:
        curves = []
        try:
            curves.append(
                _nurbs_curve_from_definition(planar_face_definition["outer"])
            )
            for hole in planar_face_definition.get("holes", []):
                curves.append(_nurbs_curve_from_definition(hole))
            breps = Rhino.Geometry.Brep.CreatePlanarBreps(
                curves, tolerance["absolute"]
            )
            if breps is None or len(breps) != 1 or not breps[0].IsValid:
                if breps is not None:
                    for brep in breps:
                        brep.Dispose()
                raise ValueError("planar-face boundary is invalid")
            return breps[0]
        finally:
            for curve in curves:
                curve.Dispose()
    box_definition = definition.get("box")
    if box_definition is not None:
        intervals = [
            Rhino.Geometry.Interval(
                _finite(box_definition[axis][0], "box boundary interval"),
                _finite(box_definition[axis][1], "box boundary interval"),
            )
            for axis in ("x", "y", "z")
        ]
        box = Rhino.Geometry.Box(
            Rhino.Geometry.Plane.WorldXY,
            intervals[0],
            intervals[1],
            intervals[2],
        )
        brep = box.ToBrep()
        if brep is None or not brep.IsValid:
            if brep is not None:
                brep.Dispose()
            raise ValueError("box boundary is invalid")
        return brep
    return _nurbs_curve_from_definition(definition)


def _surface_split_cutter_from_definition(definition, tolerance):
    if "degree_u" in definition:
        return _nurbs_surface_from_definition(definition)
    return _curve_extension_boundary_from_definition(definition, tolerance)


def _nurbs_surface_definition(surface):
    nurbs = surface.ToNurbsSurface()
    if nurbs is None:
        raise ValueError("could not convert surface to NURBS form")
    try:
        count_u = int(nurbs.Points.CountU)
        count_v = int(nurbs.Points.CountV)
        controls = []
        for v_index in range(count_v):
            for u_index in range(count_u):
                control = nurbs.Points.GetControlPoint(u_index, v_index)
                controls.append(
                    {
                        "point": _xyz(control.Location),
                        "weight": float(control.Weight),
                    }
                )
        knots_u = [float(nurbs.KnotsU[0])]
        knots_u.extend(float(nurbs.KnotsU[index]) for index in range(nurbs.KnotsU.Count))
        knots_u.append(float(nurbs.KnotsU[nurbs.KnotsU.Count - 1]))
        knots_v = [float(nurbs.KnotsV[0])]
        knots_v.extend(float(nurbs.KnotsV[index]) for index in range(nurbs.KnotsV.Count))
        knots_v.append(float(nurbs.KnotsV[nurbs.KnotsV.Count - 1]))
        return {
            "control_count": [count_u, count_v],
            "control_points": controls,
            "degree": [int(nurbs.Degree(0)), int(nurbs.Degree(1))],
            "domain_u": [float(nurbs.Domain(0).T0), float(nurbs.Domain(0).T1)],
            "domain_v": [float(nurbs.Domain(1).T0), float(nurbs.Domain(1).T1)],
            "knots_u": knots_u,
            "knots_v": knots_v,
        }
    finally:
        nurbs.Dispose()


def _mesh_fill_hole_value(mesh, source_vertex_count, source_face_count):
    patch_triangles = []
    for index in range(source_face_count, mesh.Faces.Count):
        face = mesh.Faces[index]
        if not face.IsTriangle:
            raise ValueError("mesh hole patch unexpectedly contains a quad")
        triangle = [
            int(face.A) - source_vertex_count,
            int(face.B) - source_vertex_count,
            int(face.C) - source_vertex_count,
        ]
        if any(vertex < 0 for vertex in triangle):
            raise ValueError("mesh hole patch unexpectedly reuses a source vertex")
        triangle.sort()
        patch_triangles.append(triangle)
    patch_triangles.sort()
    return {
        "added_vertices": [
            _xyz(mesh.Vertices.Point3dAt(index))
            for index in range(source_vertex_count, mesh.Vertices.Count)
        ],
        "patch_triangles": patch_triangles,
    }


def _mesh_unweld_value(mesh):
    face_points = []
    point_groups = {}
    for face_index in range(mesh.Faces.Count):
        face = mesh.Faces[face_index]
        raw_vertices = [int(face.A), int(face.B), int(face.C)]
        if not face.IsTriangle:
            raw_vertices.append(int(face.D))
        face_points.append(
            [_xyz(mesh.Vertices.Point3dAt(raw)) for raw in raw_vertices]
        )
        for raw in raw_vertices:
            point = tuple(_xyz(mesh.Vertices.Point3dAt(raw)))
            raw_groups = point_groups.setdefault(point, {})
            raw_groups.setdefault(raw, []).append(face_index)
    vertex_face_groups = []
    for point in sorted(point_groups):
        face_groups = [
            sorted(faces) for faces in point_groups[point].values()
        ]
        face_groups.sort()
        vertex_face_groups.append({
            "face_groups": face_groups,
            "point": list(point),
        })
    return {
        "face_points": face_points,
        "vertex_count": int(mesh.Vertices.Count),
        "vertex_face_groups": vertex_face_groups,
    }


def _join_close_input(definition):
    kind = definition["type"]
    if kind == "circle":
        x = Rhino.Geometry.Vector3d(*definition["x_axis"])
        normal = Rhino.Geometry.Vector3d(*definition["normal"])
        y = Rhino.Geometry.Vector3d.CrossProduct(normal, x)
        plane = Rhino.Geometry.Plane(_point(definition["center"]), x, y)
        return Rhino.Geometry.ArcCurve(Rhino.Geometry.Circle(plane, float(definition["radius"])))
    if kind == "ellipse":
        plane = Rhino.Geometry.Plane(_point(definition["center"]), Rhino.Geometry.Vector3d(*definition["x_axis"]), Rhino.Geometry.Vector3d(*definition["y_axis"]))
        return Rhino.Geometry.Ellipse(plane, float(definition["radius_x"]), float(definition["radius_y"])).ToNurbsCurve()
    if kind == "line":
        return Rhino.Geometry.LineCurve(_point(definition["start"]), _point(definition["end"]))
    if kind == "polyline":
        return Rhino.Geometry.PolylineCurve([_point(point) for point in definition["vertices"]])
    if kind == "arc":
        return Rhino.Geometry.ArcCurve(Rhino.Geometry.Arc(*[_point(point) for point in definition["points"]]))
    if kind == "nurbs":
        return _nurbs_curve_from_definition(definition)
    if kind == "polycurve":
        curve = Rhino.Geometry.PolyCurve()
        try:
            for definition in definition["segments"]:
                segment = _join_close_input(definition)
                try:
                    if not curve.AppendSegment(segment):
                        raise ValueError("could not append join/close segment")
                finally:
                    segment.Dispose()
            return curve
        except Exception:
            curve.Dispose()
            raise
    raise ValueError("unsupported join/close curve type")


def _join_close_record(curve, inspect_native=False):
    if isinstance(curve, Rhino.Geometry.PolyCurve):
        curve.RemoveNesting()
        segments = []
        for index in range(curve.SegmentCount):
            segment = curve.SegmentCurve(index).DuplicateCurve()
            try:
                segment.Domain = curve.SegmentDomain(index)
                segments.append(_nurbs_curve_definition(segment))
            finally:
                segment.Dispose()
        kind = "polycurve"
    else:
        segments = [_nurbs_curve_definition(curve)]
        kind = "nurbs"
        if isinstance(curve, Rhino.Geometry.LineCurve):
            kind = "line"
        elif isinstance(curve, Rhino.Geometry.ArcCurve):
            kind = "arc"
        elif isinstance(curve, Rhino.Geometry.PolylineCurve):
            kind = "polyline"
    value = {"type": kind, "closed": bool(curve.IsClosed), "domain": [float(curve.Domain.T0), float(curve.Domain.T1)],
            "segments": segments, "length": float(curve.GetLength(1e-12))}
    if inspect_native:
        value["native"] = _polycurve_native_record(curve, {"relative": 1e-12})
    return value


def _curve_native(operation, iterations, tolerance):
    source = _join_close_input(operation["curve"])
    try:
        def compute():
            owned = [source.DuplicateCurve()]
            try:
                curve = owned[0]
                if operation.get("domain") is not None:
                    curve.Domain = Rhino.Geometry.Interval(*operation["domain"])
                if operation.get("reversed") and not curve.Reverse():
                    raise ValueError("native curve reversal failed")
                if operation.get("transform") is not None:
                    transform = Rhino.Geometry.Transform.Identity
                    for row, values in enumerate(operation["transform"]):
                        for column, value in enumerate(values):
                            transform[row, column] = float(value)
                    # Explicitly prepare exact deformation for maps that do
                    # not preserve circles; direct ArcCurve.Transform fits a circle.
                    if transform.SimilarityType == Rhino.Geometry.TransformSimilarityType.NotSimilarity:
                        curve = curve.ToNurbsCurve()
                        if curve is None:
                            raise ValueError("could not prepare exact affine deformation")
                        owned.append(curve)
                    if not curve.Transform(transform):
                        raise ValueError("native curve transform failed")
                edit = operation.get("edit")
                if edit is None:
                    value = _curve_native_record(curve, tolerance, operation.get("differential_only", False), operation.get("sided_parameters", []))
                    if operation.get("parameter_map"):
                        mapping = []
                        for i in range(65):
                            t = float(curve.Domain.ParameterAt(float(i) / 64.0))
                            ok_n, n = curve.GetNurbsFormParameterFromCurveParameter(t)
                            ok_c, c = curve.GetCurveParameterFromNurbsFormParameter(t)
                            if not ok_n or not ok_c:
                                raise ValueError("native/rational parameter correspondence failed")
                            mapping.append({"parameter": t, "nurbs": float(n), "native": float(c)})
                        value["parameter_map"] = mapping
                    return value
                kind = edit["kind"]
                if kind == "seam":
                    if not curve.ChangeClosedCurveSeam(float(edit["parameter"])):
                        raise ValueError("native seam relocation failed")
                    curves = [curve]
                elif kind == "split":
                    curves = curve.Split(System.Array[System.Double](edit["parameters"]))
                    if curves is None:
                        raise ValueError("native multiple split failed")
                    owned.extend(curves)
                else:
                    start, end = edit["domain"]
                    reverse = kind == "subcurve" and start > end and not curve.IsClosed
                    interval = Rhino.Geometry.Interval(end, start) if reverse else Rhino.Geometry.Interval(start, end)
                    result = curve.Trim(interval)
                    if result is None:
                        raise ValueError("native curve trim failed")
                    owned.append(result)
                    if reverse and not result.Reverse():
                        raise ValueError("native subcurve reversal failed")
                    curves = [result]
                records = []
                for result in curves:
                    value = _curve_native_record(result, tolerance, operation.get("differential_only", False), operation.get("sided_parameters", []))
                    value["type"] = ("arc" if isinstance(result, Rhino.Geometry.ArcCurve) else
                                     "line" if isinstance(result, Rhino.Geometry.LineCurve) else
                                     "polyline" if isinstance(result, Rhino.Geometry.PolylineCurve) else
                                     "polycurve" if isinstance(result, Rhino.Geometry.PolyCurve) else "nurbs")
                    records.append(value)
                return {"curves": records}
            finally:
                for curve in reversed(owned):
                    curve.Dispose()
        return _measure(iterations, compute)
    finally:
        source.Dispose()


def _curve_native_record(curve, tolerance, differential_only=False, sided_parameters=()):
    samples = []
    for i in range(33):
        parameter = float(curve.Domain.ParameterAt(float(i) / 32.0))
        derivatives = curve.DerivativeAt(parameter, 2)
        if derivatives is None or len(derivatives) != 3:
            raise ValueError("native curve derivatives failed")
        samples.append({"parameter": parameter, "point": _xyz(curve.PointAt(parameter)),
                        "first": _xyz(derivatives[1]), "second": _xyz(derivatives[2]),
                        "tangent": _xyz(curve.TangentAt(parameter))})
    value = {"domain": [float(curve.Domain.T0), float(curve.Domain.T1)], "closed": bool(curve.IsClosed),
             "samples": samples, "nurbs": _nurbs_curve_definition(curve)}
    if sided_parameters:
        sides = []
        for parameter in sided_parameters:
            sample = {"parameter": float(parameter)}
            for name, side in [("left", Rhino.Geometry.CurveEvaluationSide.Below), ("right", Rhino.Geometry.CurveEvaluationSide.Above)]:
                derivatives = curve.DerivativeAt(float(parameter), 2, side)
                if derivatives is None or len(derivatives) != 3:
                    raise ValueError("one-sided curve derivatives failed")
                tangent = Rhino.Geometry.Vector3d(derivatives[1])
                if not tangent.Unitize():
                    # TangentAt has no public side argument. Restrict its
                    # domain to the selected side at a stationary point.
                    below = parameter == curve.Domain.T1 or (name == "left" and parameter > curve.Domain.T0)
                    interval = (Rhino.Geometry.Interval(curve.Domain.T0, parameter) if below else
                                Rhino.Geometry.Interval(parameter, curve.Domain.T1))
                    piece = curve.Trim(interval)
                    if piece is None:
                        raise ValueError("could not restrict stationary tangent to one side")
                    try:
                        tangent = piece.TangentAt(float(parameter))
                        if not tangent.Unitize():
                            raise ValueError("one-sided curve tangent is degenerate")
                    finally:
                        piece.Dispose()
                sample[name] = {"point": _xyz(derivatives[0]), "first": _xyz(derivatives[1]),
                                "second": _xyz(derivatives[2]), "tangent": _xyz(tangent)}
            sides.append(sample)
        value["sides"] = sides
    if differential_only:
        return value
    divisions = []
    for i in range(18):
        if i == 0:
            parameter = float(curve.Domain.T0)
        elif i == 17:
            parameter = float(curve.Domain.T1)
        else:
            success, parameter = curve.NormalizedLengthParameter(float(i) / 17.0, tolerance["relative"])
            if not success:
                raise ValueError("native curve length inversion failed")
        divisions.append({"parameter": float(parameter), "point": _xyz(curve.PointAt(parameter)), "tangent": _xyz(curve.TangentAt(parameter))})
    value["length"] = float(curve.GetLength(tolerance["relative"]))
    value["divisions"] = divisions
    return value

def _cut_source(definition):
    if "native" not in definition:
        return _nurbs_curve_from_definition(definition)
    curve = _join_close_input(definition["native"])
    try:
        if definition.get("domain") is not None:
            curve.Domain = Rhino.Geometry.Interval(*definition["domain"])
        if definition.get("reversed") and not curve.Reverse():
            raise ValueError("cut source reversal failed")
        return curve
    except Exception:
        curve.Dispose()
        raise


def _cut_native_record(curve):
    kind = ("arc" if isinstance(curve, Rhino.Geometry.ArcCurve) else
            "line" if isinstance(curve, Rhino.Geometry.LineCurve) else
            "polyline" if isinstance(curve, Rhino.Geometry.PolylineCurve) else
            "polycurve" if isinstance(curve, Rhino.Geometry.PolyCurve) else "nurbs")
    return {"type": kind, "domain": [float(curve.Domain.T0), float(curve.Domain.T1)],
            "points": [_xyz(curve.PointAt(curve.Domain.ParameterAt(float(i) / 16.0))) for i in range(17)]}


def _polycurve_native_record(curve, tolerance):
    segments = []
    count = curve.SegmentCount if isinstance(curve, Rhino.Geometry.PolyCurve) else 1
    for index in range(count):
        segment = curve.SegmentCurve(index).DuplicateCurve() if isinstance(curve, Rhino.Geometry.PolyCurve) else curve.DuplicateCurve()
        try:
            domain = curve.SegmentDomain(index) if isinstance(curve, Rhino.Geometry.PolyCurve) else curve.Domain
            segment.Domain = domain
            kind = "nurbs"
            if isinstance(segment, Rhino.Geometry.LineCurve):
                kind = "line"
            elif isinstance(segment, Rhino.Geometry.ArcCurve):
                kind = "arc"
            elif isinstance(segment, Rhino.Geometry.PolylineCurve):
                kind = "polyline"
            samples = []
            for fraction in [0.0, 0.125, 0.375, 0.5, 0.875, 1.0]:
                parameter = float(domain.T1) if fraction == 1.0 else float(domain.T0 + domain.Length * fraction)
                derivatives = segment.DerivativeAt(parameter, 2)
                if derivatives is None or len(derivatives) != 3:
                    raise ValueError("could not evaluate native segment derivatives")
                samples.append({"parameter": parameter, "point": _xyz(segment.PointAt(parameter)),
                                "first": _xyz(derivatives[1]), "second": _xyz(derivatives[2])})
            segments.append({"type": kind, "domain": [float(domain.T0), float(domain.T1)],
                             "samples": samples, "nurbs": _nurbs_curve_definition(segment)})
        finally:
            segment.Dispose()
    return {"domain": [float(curve.Domain.T0), float(curve.Domain.T1)], "closed": bool(curve.IsClosed),
            "length": float(curve.GetLength(tolerance["relative"])), "segments": segments}


def _polycurve_native(operation, iterations, tolerance):
    source = _join_close_input(operation["curve"])
    try:
        if not isinstance(source, Rhino.Geometry.PolyCurve) or not source.IsValid:
            raise ValueError("native polycurve fixture requires a valid composite")
        source.RemoveNesting()
        def compute():
            owned = [source.DuplicateCurve()]
            try:
                curve = owned[0]
                if operation.get("domain") is not None:
                    curve.Domain = Rhino.Geometry.Interval(*operation["domain"])
                if operation.get("reversed") and not curve.Reverse():
                    raise ValueError("native reverse failed")
                if operation.get("trim") is not None:
                    curve = curve.Trim(*operation["trim"])
                    if curve is None:
                        raise ValueError("native trim failed")
                    owned.append(curve)
                if operation.get("deformable") and not curve.MakeDeformable():
                    raise ValueError("could not make native polycurve deformable")
                if operation.get("transform") is not None:
                    transform = Rhino.Geometry.Transform.Identity
                    for row, values in enumerate(operation["transform"]):
                        for column, value in enumerate(values):
                            transform[row, column] = float(value)
                    if not curve.Transform(transform):
                        raise ValueError("native transform failed")
                curves = [curve]
                if operation.get("split") is not None:
                    curves = curve.Split(float(operation["split"]))
                    if curves is None or len(curves) != 2:
                        raise ValueError("native split failed")
                    owned.extend(curves)
                results = []
                for c in curves:
                    value = _polycurve_native_record(c, tolerance)
                    if operation.get("document_checks"):
                        value["document"] = _polycurve_document_record(c)
                    results.append(value)
                return {"curves": results}
            finally:
                for curve in reversed(owned):
                    curve.Dispose()
        return _measure(iterations, compute)
    finally:
        source.Dispose()


def _curve_join_close(operation, iterations, tolerance):
    curves = [_join_close_input(definition) for definition in operation["curves"]]
    try:
        if not all(curve.IsValid for curve in curves):
            raise ValueError("invalid join/close source curve")
        if operation["action"] == "join":
            def compute():
                results = Rhino.Geometry.Curve.JoinCurves(curves, operation.get("join_tolerance", tolerance["absolute"]), operation.get("preserve_direction", False))
                if results is None:
                    raise ValueError("JoinCurves failed")
                try:
                    return [_join_close_record(curve, operation.get("inspect_native", False)) for curve in results]
                finally:
                    for curve in results:
                        curve.Dispose()
        else:
            def compute():
                document = Rhino.RhinoDoc.ActiveDoc
                settings = Rhino.DocObjects.ObjectEnumeratorSettings()
                settings.NormalObjects = True
                before = set(obj.Id for obj in document.Objects.GetObjectList(settings))
                document.Objects.UnselectAll()
                ids = [document.Objects.AddCurve(curve) for curve in curves]
                groups = {}
                try:
                    selectors = " ".join("_SelID %s" % object_id for object_id in ids)
                    if operation["action"] == "join_command":
                        for index, object_id in enumerate(ids):
                            group = document.Groups.Add()
                            groups[group] = "source-%d" % index
                            document.Groups.AddToGroup(group, object_id)
                            obj = document.Objects.FindId(object_id)
                            attributes = obj.Attributes.Duplicate()
                            attributes.Name = "source-%d" % index
                            document.Objects.ModifyAttributes(object_id, attributes, True)
                        group = document.Groups.Add()
                        groups[group] = "shared"
                        for object_id in ids:
                            document.Groups.AddToGroup(group, object_id)
                        macro = "_-Join %s _Enter" % selectors
                    else:
                        macro = "_-CloseCrv _CloseWideGapsWithLine=%s _Tolerance=%.17g %s _Enter" % (
                            "Yes" if operation.get("close_wide_gaps_with_line", True) else "No",
                            operation.get("close_tolerance", tolerance["absolute"]), selectors)
                    succeeded = bool(Rhino.RhinoApp.RunScript(macro, False))
                    results = []
                    for obj in document.Objects.GetObjectList(settings):
                        if obj.Id not in before:
                            duplicate = obj.Geometry.DuplicateCurve()
                            try:
                                value = _join_close_record(duplicate, operation.get("inspect_native", False))
                                if operation["action"] == "join_command":
                                    value["name"] = obj.Attributes.Name
                                    value["source_index"] = ids.index(obj.Id) if obj.Id in ids else None
                                    value["groups"] = sorted(groups[index] for index in (obj.Attributes.GetGroupList() or []) if index in groups)
                                results.append(value)
                            finally:
                                duplicate.Dispose()
                    if operation["action"] == "join_command":
                        results.sort(key=lambda value: value["name"])
                    return {"succeeded": succeeded, "curves": results}
                finally:
                    document.Objects.UnselectAll()
                    for obj in list(document.Objects.GetObjectList(settings)):
                        if obj.Id not in before:
                            document.Objects.Delete(obj.Id, True)
                    for group in groups:
                        document.Groups.Delete(group)
        return _measure(iterations, compute)
    finally:
        for curve in curves:
            curve.Dispose()


def _interchange_curve_record(curve):
    if not curve.IsValid:
        raise ValueError("Viboceros exported an invalid Rhino curve")
    classes = [
        (Rhino.Geometry.PolyCurve, "polycurve"),
        (Rhino.Geometry.NurbsCurve, "nurbs"),
        (Rhino.Geometry.LineCurve, "line"),
        (Rhino.Geometry.ArcCurve, "arc"),
        (Rhino.Geometry.PolylineCurve, "polyline"),
    ]
    kind = next((name for cls, name in classes if isinstance(curve, cls)), None)
    if kind is None:
        raise ValueError("unexpected interchange curve type")
    value = {
        "type": kind,
        "domain": [float(curve.Domain.T0), float(curve.Domain.T1)],
        "closed": bool(curve.IsClosed),
        "samples": [_xyz(curve.PointAt(curve.Domain.ParameterAt(i / 32.0))) for i in range(33)],
    }
    if kind == "nurbs":
        value["definition"] = _nurbs_curve_definition(curve)
    if kind == "polycurve":
        value["parameters"] = [float(curve.SegmentDomain(0).T0)] + [
            float(curve.SegmentDomain(i).T1) for i in range(curve.SegmentCount)
        ]
        value["segments"] = [
            _interchange_curve_record(curve.SegmentCurve(i)) for i in range(curve.SegmentCount)
        ]
    return value


def _three_dm_curve_interchange(operation, iterations):
    path = operation.get("artifact_path")
    if not path:
        raise ValueError("three_dm_curve_interchange requires compare mode to create the shared file")
    if path.startswith("/"):
        path = "Z:" + path.replace("/", "\\")

    def read():
        model = Rhino.FileIO.File3dm.Read(path)
        if model is None:
            raise ValueError("Rhino could not read the Viboceros-written file")
        try:
            objects = []
            for item in model.Objects:
                a = item.Attributes
                color = a.ObjectColor
                groups = a.GetGroupList()
                objects.append({
                    "name": a.Name, "visible": bool(a.Visible),
                    "locked": a.Mode == Rhino.DocObjects.ObjectMode.Locked,
                    "color": [int(color.R), int(color.G), int(color.B)],
                    "color_source": int(a.ColorSource), "wire_density": int(a.WireDensity),
                    "groups": [] if groups is None else [int(i) for i in groups],
                    "layer": int(a.LayerIndex), "curve": _interchange_curve_record(item.Geometry),
                })
            layers = []
            for layer in model.AllLayers:
                color = layer.Color
                layers.append({
                    "name": layer.Name, "color": [int(color.R), int(color.G), int(color.B)],
                    "visible": bool(layer.IsVisible), "locked": bool(layer.IsLocked),
                })
            return {
                "groups": [g.Name for g in model.AllGroups],
                "layers": layers, "objects": objects,
            }
        finally:
            model.Dispose()

    return _measure(iterations, read)


def _surface_jets(operation, iterations):
    surface = _nurbs_surface_from_definition(operation["surface"])
    owned = [surface]
    try:
        for axis, name in [(0, "reverse_u"), (1, "reverse_v")]:
            if operation.get(name, False):
                result = surface.Reverse(axis)
                if result is None:
                    raise ValueError("surface reversal failed")
                owned.append(result)
                surface = result
        if operation.get("swap_uv", False):
            result = surface.Transpose()
            if result is None:
                raise ValueError("surface transpose failed")
            owned.append(result)
            surface = result
        if operation.get("translation") is not None:
            if not surface.Transform(Rhino.Geometry.Transform.Translation(_vector(operation["translation"]))):
                raise ValueError("surface translation failed")
        domains = [surface.Domain(0), surface.Domain(1)]
        samples = operation.get("samples")
        if samples is None:
            samples = [{"parameter": [domains[0].ParameterAt(i / 4.0), domains[1].ParameterAt(j / 4.0)]}
                       for j in range(5) for i in range(5)]
        if not samples:
            raise ValueError("surface jets need samples")
        prepared = []
        extended = operation.get("extended", False)
        for sample in samples:
            uv = [_finite(t, "surface jet parameter") for t in sample["parameter"]]
            if len(uv) != 2:
                raise ValueError("surface jet needs two parameters")
            intervals = []
            trim = False
            explicit_sides = "side_u" in sample or "side_v" in sample
            for axis, key in [(0, "side_u"), (1, "side_v")]:
                side = sample.get(key, "right")
                if side not in ("left", "right") or (extended and side == "left"):
                    raise ValueError("invalid surface evaluation side")
                domain = domains[axis]
                t = uv[axis]
                if not extended and not domain.T0 <= t <= domain.T1:
                    raise ValueError("surface parameter is outside its domain")
                # RhinoCommon Evaluate has no quadrant argument. Exact trimming
                # makes the selected span the sole interior boundary limit;
                # no parameter perturbation or finite difference is involved.
                if explicit_sides and not extended and domain.T0 < t < domain.T1:
                    intervals.append(Rhino.Geometry.Interval(domain.T0, t) if side == "left"
                                     else Rhino.Geometry.Interval(t, domain.T1))
                    trim = True
                else:
                    intervals.append(domain)
            target = surface
            if trim:
                target = surface.Trim(intervals[0], intervals[1])
                if target is None:
                    raise ValueError("could not isolate surface evaluation quadrant")
                owned.append(target)
            prepared.append((target, uv))

        def compute():
            values = []
            for target, uv in prepared:
                success, point, derivatives = target.Evaluate(uv[0], uv[1], 2)
                if not success or derivatives is None or len(derivatives) != 5:
                    raise ValueError("Rhino could not evaluate second surface partials")
                values.append((point, derivatives))
            return values

        jets, elapsed = _measure(iterations, compute)
        records = []
        for (_, uv), (point, derivatives) in zip(prepared, jets):
            record = {"parameter": uv, "point": _xyz(point)}
            for key, derivative in zip(["du", "dv", "duu", "duv", "dvv"], derivatives):
                record[key] = _xyz(derivative)
            records.append(record)
        return {"domain_u": [float(domains[0].T0), float(domains[0].T1)],
                "domain_v": [float(domains[1].T0), float(domains[1].T1)],
                "samples": records}, elapsed
    finally:
        for item in reversed(owned):
            item.Dispose()


def _execute(operation, iterations, tolerance):
    kind = operation["op"]
    if kind == "surface_jets":
        return _surface_jets(operation, iterations)
    if kind == "three_dm_curve_interchange":
        return _three_dm_curve_interchange(operation, iterations)
    if kind == "curve_join_close":
        return _curve_join_close(operation, iterations, tolerance)
    if kind == "polycurve_native":
        return _polycurve_native(operation, iterations, tolerance)
    if kind == "curve_native":
        return _curve_native(operation, iterations, tolerance)
    if kind in ("polycurve_geometry", "polycurve_document"):
        return _polycurve_geometry(operation, iterations, tolerance)
    if kind == "trimmed_surface_mass_properties":
        return _trimmed_surface_mass_properties(operation, iterations, tolerance)
    if kind == "mesh_weld_vertex":
        document = Rhino.RhinoDoc.ActiveDoc
        source = _triangle_mesh(operation["vertices"], operation["triangles"])
        vertex_indices = operation["vertex_indices"]
        if not isinstance(vertex_indices, list) or any(
            isinstance(index, bool) or int(index) != index
            for index in vertex_indices
        ):
            source.Dispose()
            raise ValueError("mesh weld vertex indices must be integers")
        vertex_indices = [int(index) for index in vertex_indices]
        before = int(source.Vertices.Count)
        if not vertex_indices:
            try:
                return ({
                    "accepted": False,
                    "removed_vertices": 0,
                    "mesh": _mesh_unweld_value(source),
                }, 0)
            finally:
                source.Dispose()
        source.Dispose()

        def weld_mesh_vertices():
            command_source = _triangle_mesh(
                operation["vertices"], operation["triangles"]
            )
            try:
                object_id = document.Objects.AddMesh(command_source)
            finally:
                command_source.Dispose()
            if object_id == System.Guid.Empty:
                raise ValueError("could not add mesh weld vertex oracle source")
            document.Objects.UnselectAll()
            mesh_object = document.Objects.FindId(object_id)
            try:
                for index in vertex_indices:
                    component = Rhino.Geometry.ComponentIndex(
                        Rhino.Geometry.ComponentIndexType.MeshTopologyVertex,
                        index,
                    )
                    if mesh_object.SelectSubObject(component, True, True, False) == 0:
                        raise ValueError("could not select mesh topology vertex")
                # As with UnweldVertex, RunScript can report false for a
                # command nested inside the Python oracle even after its
                # synchronous topology edit completed.
                Rhino.RhinoApp.RunScript("_-WeldVertices _Enter", False)
                mesh_object = document.Objects.FindId(object_id)
                if mesh_object is None:
                    raise ValueError("mesh weld vertex command removed its source")
                return {
                    "accepted": True,
                    "removed_vertices": before - int(mesh_object.Geometry.Vertices.Count),
                    "mesh": _mesh_unweld_value(mesh_object.Geometry),
                }
            finally:
                document.Objects.UnselectAll()
                document.Objects.Delete(object_id, True)

        return _measure(iterations, weld_mesh_vertices)

    if kind == "mesh_weld_edge":
        document = Rhino.RhinoDoc.ActiveDoc
        source = _triangle_mesh(operation["vertices"], operation["triangles"])
        edge_indices = operation["edge_indices"]
        if not isinstance(edge_indices, list) or any(
            isinstance(index, bool) or int(index) != index
            for index in edge_indices
        ):
            source.Dispose()
            raise ValueError("mesh weld edge indices must be integers")
        edge_indices = [int(index) for index in edge_indices]
        before = int(source.Vertices.Count)
        if not edge_indices:
            try:
                return ({
                    "accepted": False,
                    "removed_vertices": 0,
                    "mesh": _mesh_unweld_value(source),
                }, 0)
            finally:
                source.Dispose()
        source.Dispose()

        def weld_mesh_edges():
            command_source = _triangle_mesh(
                operation["vertices"], operation["triangles"]
            )
            try:
                object_id = document.Objects.AddMesh(command_source)
            finally:
                command_source.Dispose()
            if object_id == System.Guid.Empty:
                raise ValueError("could not add mesh weld edge oracle source")
            document.Objects.UnselectAll()
            mesh_object = document.Objects.FindId(object_id)
            try:
                for index in edge_indices:
                    component = Rhino.Geometry.ComponentIndex(
                        Rhino.Geometry.ComponentIndexType.MeshTopologyEdge,
                        index,
                    )
                    if mesh_object.SelectSubObject(component, True, True, False) == 0:
                        raise ValueError("could not select mesh topology edge")
                if not Rhino.RhinoApp.RunScript("_-WeldEdge _Enter", False):
                    raise ValueError("mesh weld edge command failed")
                mesh_object = document.Objects.FindId(object_id)
                if mesh_object is None:
                    raise ValueError("mesh weld edge command removed its source")
                return {
                    "accepted": True,
                    "removed_vertices": before - int(mesh_object.Geometry.Vertices.Count),
                    "mesh": _mesh_unweld_value(mesh_object.Geometry),
                }
            finally:
                document.Objects.UnselectAll()
                document.Objects.Delete(object_id, True)

        return _measure(iterations, weld_mesh_edges)

    if kind == "mesh_unweld_vertex":
        document = Rhino.RhinoDoc.ActiveDoc
        source = _triangle_mesh(operation["vertices"], operation["triangles"])
        vertex_indices = operation["vertex_indices"]
        if not isinstance(vertex_indices, list) or any(
            isinstance(index, bool) or int(index) != index
            for index in vertex_indices
        ):
            source.Dispose()
            raise ValueError("mesh unweld vertex indices must be integers")
        vertex_indices = [int(index) for index in vertex_indices]
        modify_normals = operation["modify_normals"]
        if not isinstance(modify_normals, bool):
            source.Dispose()
            raise ValueError("mesh unweld vertex modify_normals must be a boolean")
        before = int(source.Vertices.Count)

        if not vertex_indices:
            try:
                return ({
                    "accepted": False,
                    "added_vertices": 0,
                    "mesh": _mesh_unweld_value(source),
                }, 0)
            finally:
                source.Dispose()

        source.Dispose()

        def unweld_mesh_vertices():
            command_source = _triangle_mesh(
                operation["vertices"], operation["triangles"]
            )
            try:
                object_id = document.Objects.AddMesh(command_source)
            finally:
                command_source.Dispose()
            if object_id == System.Guid.Empty:
                raise ValueError("could not add mesh unweld vertex oracle source")
            document.Objects.UnselectAll()
            mesh_object = document.Objects.FindId(object_id)
            try:
                for index in vertex_indices:
                    component = Rhino.Geometry.ComponentIndex(
                        Rhino.Geometry.ComponentIndexType.MeshTopologyVertex,
                        index,
                    )
                    if mesh_object.SelectSubObject(component, True, True, False) == 0:
                        raise ValueError("could not select mesh topology vertex")
                command = "_-UnweldVertex _ModifyNormals=_%s _Enter" % (
                    "Yes" if modify_normals else "No"
                )
                # RunScript reports false when nested inside the oracle's
                # Python command, but the documented command completes its
                # synchronous topology edit before returning.
                Rhino.RhinoApp.RunScript(command, False)
                mesh_object = document.Objects.FindId(object_id)
                if mesh_object is None:
                    raise ValueError("mesh unweld vertex command removed its source")
                return {
                    "accepted": True,
                    "added_vertices": int(mesh_object.Geometry.Vertices.Count) - before,
                    "mesh": _mesh_unweld_value(mesh_object.Geometry),
                }
            finally:
                document.Objects.UnselectAll()
                document.Objects.Delete(object_id, True)

        return _measure(iterations, unweld_mesh_vertices)

    if kind == "document_surface_orient_cycle":
        document = Rhino.RhinoDoc.ActiveDoc
        suffix = str(System.Guid.NewGuid())
        name_prefix = "Viboceros Surface Orient " + suffix + " "
        fixture_group_indices = set()
        target_ids = []

        def fixture_number(value):
            value = round(float(value), 6)
            return 0.0 if value == 0.0 else value

        def fixture_point(value):
            return [
                fixture_number(value.X),
                fixture_number(value.Y),
                fixture_number(value.Z),
            ]

        def fixture_objects():
            return [
                rhino_object
                for rhino_object in document.Objects
                if rhino_object.Attributes.Name is not None
                and rhino_object.Attributes.Name.startswith(name_prefix)
            ]

        def curve_record(rhino_object, scenario_prefix):
            curve = rhino_object.Geometry
            if not isinstance(curve, Rhino.Geometry.Curve):
                raise ValueError("surface-orient fixture contains a non-curve")
            nurbs = curve.ToNurbsCurve()
            if nurbs is None:
                raise ValueError("could not convert surface-orient curve to NURBS")
            return {
                "controls": [
                    {
                        "point": fixture_point(nurbs.Points[index].Location),
                        "weight": fixture_number(nurbs.Points[index].Weight),
                    }
                    for index in range(nurbs.Points.Count)
                ],
                "degree": int(nurbs.Degree),
                "is_rational": bool(nurbs.IsRational),
                "name": rhino_object.Attributes.Name[len(scenario_prefix):],
                "selected": rhino_object.IsSelected(False) > 0,
            }

        def record_key(record):
            first = (
                record["controls"][0]["point"]
                if record["controls"]
                else [0.0, 0.0, 0.0]
            )
            return tuple([record["name"], record["degree"]] + first)

        def scenario_groups(objects, scenario_prefix):
            fixture_ids = set(item.Id for item in objects)
            groups = []
            for group_index in range(document.Groups.Count):
                if document.Groups.IsDeleted(group_index):
                    continue
                members = document.Groups.GroupMembers(group_index)
                if members is None:
                    continue
                records = [
                    curve_record(member, scenario_prefix)
                    for member in members
                    if member.Id in fixture_ids
                ]
                if records:
                    fixture_group_indices.add(group_index)
                    records.sort(key=record_key)
                    groups.append(records)
            groups.sort(key=lambda group: tuple(record_key(item) for item in group))
            return groups

        def create_surface(surface_kind):
            if surface_kind == "bilinear":
                surface = Rhino.Geometry.NurbsSurface.Create(
                    3, False, 2, 2, 2, 2
                )
                controls = [
                    {"point": [0.0, 0.0, 0.0]},
                    {"point": [10.0, 0.0, 0.0]},
                    {"point": [0.0, 10.0, 10.0]},
                    {"point": [12.0, 10.0, 10.0]},
                ]
                control_count_u = 2
                u_knots = [0.0, 0.0, 1.0, 1.0]
                v_knots = [0.0, 0.0, 1.0, 1.0]
            else:
                surface = Rhino.Geometry.NurbsSurface.Create(
                    3, surface_kind == "cylinder", 3, 2, 3, 2
                )
                control_count_u = 3
                u_knots = [0.0, 0.0, 0.0, 1.0, 1.0, 1.0]
                v_knots = [0.0, 0.0, 1.0, 1.0]
            if surface is None:
                raise ValueError("could not allocate surface-orient fixture")
            if surface_kind == "cylinder":
                middle_weight = math.sqrt(0.5)
                controls = [
                    {"point": [10.0, 0.0, 0.0], "weight": 1.0},
                    {"point": [10.0, 10.0, 0.0], "weight": middle_weight},
                    {"point": [0.0, 10.0, 0.0], "weight": 1.0},
                    {"point": [10.0, 0.0, 10.0], "weight": 1.0},
                    {"point": [10.0, 10.0, 10.0], "weight": middle_weight},
                    {"point": [0.0, 10.0, 10.0], "weight": 1.0},
                ]
            elif surface_kind != "bilinear":
                controls = [
                    {"point": [0.0, 0.0, 0.0]},
                    {"point": [5.0, 0.0, 0.0]},
                    {"point": [10.0, 0.0, 0.0]},
                    {"point": [0.0, 10.0, 10.0]},
                    {"point": [0.0, 20.0, 10.0]},
                    {"point": [10.0, 10.0, 10.0]},
                ]
            _set_surface_controls(surface, controls, control_count_u, 2)
            _set_knots(
                surface.KnotsU,
                u_knots,
                "surface-orient U knot",
            )
            _set_knots(
                surface.KnotsV,
                v_knots,
                "surface-orient V knot",
            )
            return surface

        def run_scenario(
            label,
            surface_kind,
            reference_point,
            copy,
            rigid,
            flip,
            scale,
            rotation,
        ):
            _record_progress("document_surface_orient_cycle: %s start" % label)
            origin = Rhino.Geometry.Point3d(1.0, 2.0, 3.0)
            source_ids = []
            scenario_prefix = name_prefix + label + " "
            for axis, offset in (
                ("x", Rhino.Geometry.Vector3d(1.0, 0.0, 0.0)),
                ("y", Rhino.Geometry.Vector3d(0.0, 1.0, 0.0)),
                ("z", Rhino.Geometry.Vector3d(0.0, 0.0, 1.0)),
            ):
                attributes = Rhino.DocObjects.ObjectAttributes()
                attributes.Name = scenario_prefix + axis
                source_id = document.Objects.AddLine(origin, origin + offset, attributes)
                if source_id == System.Guid.Empty:
                    raise ValueError("could not add surface-orient fixture line")
                source_ids.append(source_id)
            group_index = document.Groups.Add(
                "Viboceros Surface Orient Group " + suffix + " " + label,
                source_ids,
            )
            if group_index < 0:
                raise ValueError("could not group surface-orient fixture lines")
            fixture_group_indices.add(group_index)
            surface = create_surface(surface_kind)
            if surface is None or not surface.IsValid:
                raise ValueError("surface-orient fixture surface is invalid")
            target_id = document.Objects.AddSurface(surface)
            if target_id == System.Guid.Empty:
                raise ValueError("could not add surface-orient target")
            target_ids.append(target_id)
            target_point = surface.PointAt(0.3, 0.4)
            document.Objects.UnselectAll()
            for source_id in source_ids:
                document.Objects.Select(source_id)
            command = (
                "_-OrientOnSrf 1,2,3 %s '_-SelID %s "
                "_Copy=_%s _Rigid=_%s _Flip=_%s %.17g,%.17g,%.17g %.17g %.17g _Enter"
                % (
                    reference_point,
                    target_id,
                    "Yes" if copy else "No",
                    "Yes" if rigid else "No",
                    "Yes" if flip else "No",
                    target_point.X,
                    target_point.Y,
                    target_point.Z,
                    scale,
                    rotation,
                )
            )
            succeeded = Rhino.RhinoApp.RunScript(command, False)
            _record_progress(
                "document_surface_orient_cycle: %s command complete" % label
            )
            objects = [
                item
                for item in fixture_objects()
                if item.Attributes.Name.startswith(scenario_prefix)
            ]
            expected_count = len(source_ids) * (2 if copy else 1)
            if len(objects) != expected_count:
                history = Rhino.RhinoApp.CommandHistoryWindowText
                raise ValueError(
                    "OrientOnSrf macro %r returned %r and left %d fixture objects; "
                    "history tail: %s"
                    % (command, succeeded, len(objects), history[-2000:])
                )
            records = [curve_record(item, scenario_prefix) for item in objects]
            records.sort(key=record_key)
            return {
                "command_succeeded": bool(succeeded),
                "groups": scenario_groups(objects, scenario_prefix),
                "objects": records,
                "originals_selected": [
                    index
                    for index, source_id in enumerate(source_ids)
                    if document.Objects.FindId(source_id).IsSelected(False) > 0
                ],
                "surface_selected": (
                    document.Objects.FindId(target_id).IsSelected(False) > 0
                ),
            }

        try:
            value = {
                "deformable": run_scenario(
                    "deformable", "cylinder", "2,2,3", True, False, False, 1.0, 0.0
                ),
                "flip": run_scenario(
                    "flip", "cylinder", "2,2,3", True, False, True, 1.0, 0.0
                ),
                "scale_rotate": run_scenario(
                    "scale-rotate", "cylinder", "2,2,3", True, False, False, 2.0, 90.0
                ),
                "rigid": run_scenario(
                    "rigid", "cylinder", "2,2,3", True, True, False, 2.0, 35.0
                ),
                "oblique_source": run_scenario(
                    "oblique-source", "bilinear", "2,3,4", True, False, False, 1.0, 0.0
                ),
                "copy_no": run_scenario(
                    "copy-no", "warped", "2,2,3", False, False, False, 1.0, 0.0
                ),
            }
            timing_surface = create_surface("cylinder")
            timing_plane = Rhino.Geometry.Plane(
                Rhino.Geometry.Point3d(1.0, 2.0, 3.0),
                Rhino.Geometry.Vector3d.XAxis,
                Rhino.Geometry.Vector3d.YAxis,
            )
            timing_morph = Rhino.Geometry.Morphs.SplopSpaceMorph(
                timing_plane,
                timing_surface,
                Rhino.Geometry.Point2d(0.3, 0.4),
            )
            try:
                timing_point = Rhino.Geometry.Point3d(3.0, 0.5, 3.75)
                _unused, elapsed = _measure(
                    iterations, lambda: timing_morph.MorphPoint(timing_point)
                )
            finally:
                timing_morph.Dispose()
            return value, elapsed
        finally:
            document.Objects.UnselectAll()
            objects = fixture_objects()
            for group_index in sorted(fixture_group_indices, reverse=True):
                if not document.Groups.IsDeleted(group_index):
                    document.Groups.Delete(group_index)
            for item in objects:
                document.Objects.Delete(item.Id, True)
            for target_id in target_ids:
                document.Objects.Delete(target_id, True)
    if kind == "document_surface_array_cycle":
        document = Rhino.RhinoDoc.ActiveDoc
        suffix = str(System.Guid.NewGuid())
        name_prefix = "Viboceros Surface Array " + suffix + " "
        fixture_group_indices = set()
        target_ids = []

        def fixture_xyz(value):
            coordinates = [round(float(component), 6) for component in value]
            return [0.0 if component == 0.0 else component for component in coordinates]

        def fixture_objects():
            objects = []
            for rhino_object in document.Objects:
                name = rhino_object.Attributes.Name
                if name is not None and name.startswith(name_prefix):
                    objects.append(rhino_object)
            return objects

        def line_record(rhino_object):
            geometry = rhino_object.Geometry
            return {
                "end": fixture_xyz(geometry.PointAtEnd),
                "name": rhino_object.Attributes.Name[len(name_prefix):],
                "selected": rhino_object.IsSelected(False) > 0,
                "start": fixture_xyz(geometry.PointAtStart),
            }

        def record_key(record):
            return tuple(record["start"] + record["end"] + [record["name"]])

        def scenario_groups(objects):
            fixture_ids = set(item.Id for item in objects)
            groups = []
            for group_index in range(document.Groups.Count):
                if document.Groups.IsDeleted(group_index):
                    continue
                members = document.Groups.GroupMembers(group_index)
                if members is None:
                    continue
                records = [
                    line_record(member)
                    for member in members
                    if member.Id in fixture_ids
                ]
                if records:
                    fixture_group_indices.add(group_index)
                    records.sort(key=record_key)
                    groups.append(records)
            groups.sort(
                key=lambda group: tuple(record_key(record) for record in group)
            )
            return groups

        def create_surface(surface_kind):
            if surface_kind == "bilinear":
                return Rhino.Geometry.NurbsSurface.CreateFromCorners(
                    Rhino.Geometry.Point3d(0.0, 0.0, 0.0),
                    Rhino.Geometry.Point3d(10.0, 0.0, 0.0),
                    Rhino.Geometry.Point3d(12.0, 10.0, 10.0),
                    Rhino.Geometry.Point3d(0.0, 10.0, 10.0),
                )
            surface = Rhino.Geometry.NurbsSurface.Create(
                3, surface_kind == "cylinder", 3, 2, 3, 2
            )
            if surface is None:
                raise ValueError("could not allocate surface-array fixture")
            if surface_kind == "cylinder":
                middle_weight = math.sqrt(0.5)
                controls = [
                    {"point": [10.0, 0.0, 0.0], "weight": 1.0},
                    {"point": [10.0, 10.0, 0.0], "weight": middle_weight},
                    {"point": [0.0, 10.0, 0.0], "weight": 1.0},
                    {"point": [10.0, 0.0, 10.0], "weight": 1.0},
                    {"point": [10.0, 10.0, 10.0], "weight": middle_weight},
                    {"point": [0.0, 10.0, 10.0], "weight": 1.0},
                ]
            else:
                controls = [
                    {"point": [0.0, 0.0, 0.0]},
                    {"point": [5.0, 0.0, 0.0]},
                    {"point": [10.0, 0.0, 0.0]},
                    {"point": [0.0, 10.0, 10.0]},
                    {"point": [0.0, 20.0, 10.0]},
                    {"point": [10.0, 10.0, 10.0]},
                ]
            _set_surface_controls(surface, controls, 3, 2)
            _set_knots(
                surface.KnotsU,
                [0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                "surface-array U knot",
            )
            _set_knots(
                surface.KnotsV,
                [0.0, 0.0, 1.0, 1.0],
                "surface-array V knot",
            )
            return surface

        def run_scenario(
            label, surface_kind, command_template, u_count, v_count
        ):
            _record_progress("document_surface_array_cycle: %s start" % label)
            origin = Rhino.Geometry.Point3d(1.0, 2.0, 3.0)
            source_ids = []
            for axis, offset in (
                ("x", Rhino.Geometry.Vector3d(1.0, 0.0, 0.0)),
                ("y", Rhino.Geometry.Vector3d(0.0, 1.0, 0.0)),
                ("z", Rhino.Geometry.Vector3d(0.0, 0.0, 1.0)),
            ):
                attributes = Rhino.DocObjects.ObjectAttributes()
                attributes.Name = name_prefix + label + " " + axis
                source_id = document.Objects.AddLine(
                    origin, origin + offset, attributes
                )
                if source_id == System.Guid.Empty:
                    raise ValueError("could not add surface-array fixture line")
                source_ids.append(source_id)
            group_index = document.Groups.Add(
                "Viboceros Surface Array Group " + suffix + " " + label,
                source_ids,
            )
            if group_index < 0:
                raise ValueError("could not group surface-array fixture lines")
            fixture_group_indices.add(group_index)
            surface = create_surface(surface_kind)
            if surface is None or not surface.IsValid:
                raise ValueError("surface-array fixture surface is invalid")
            target_id = document.Objects.AddSurface(surface)
            if target_id == System.Guid.Empty:
                raise ValueError("could not add surface-array fixture surface")
            target_ids.append(target_id)
            document.Objects.UnselectAll()
            for source_id in source_ids:
                document.Objects.Select(source_id)
            command = command_template.replace("{target_id}", str(target_id))
            succeeded = Rhino.RhinoApp.RunScript(command, False)
            _record_progress(
                "document_surface_array_cycle: %s command complete" % label
            )
            scenario_prefix = name_prefix + label + " "
            objects = [
                item
                for item in fixture_objects()
                if item.Attributes.Name.startswith(scenario_prefix)
            ]
            expected_count = len(source_ids) * (1 + u_count * v_count)
            if len(objects) != expected_count:
                history = Rhino.RhinoApp.CommandHistoryWindowText
                raise ValueError(
                    "ArraySrf macro %r returned %r and left %d fixture objects; "
                    "history tail: %s"
                    % (
                        command,
                        succeeded,
                        len(objects),
                        history[-2000:],
                    )
                )
            records = [line_record(item) for item in objects]
            records.sort(key=record_key)
            return {
                "command_succeeded": bool(succeeded),
                "groups": scenario_groups(objects),
                "objects": records,
                "originals_selected": [
                    index
                    for index, source_id in enumerate(source_ids)
                    if document.Objects.FindId(source_id).IsSelected(False) > 0
                ],
                "surface_selected": (
                    document.Objects.FindId(target_id).IsSelected(False) > 0
                ),
            }

        try:
            base = "_-ArraySrf _Mode={mode} 1,2,3 {up} '_-SelID {target_id} "
            value = {
                "uv": run_scenario(
                    "uv",
                    "bilinear",
                    base.format(mode="_UV", up="_Enter", target_id="{target_id}")
                    + "3 2 _Enter",
                    3,
                    2,
                ),
                "cylinder_uv": run_scenario(
                    "cylinder-uv",
                    "cylinder",
                    base.format(mode="_UV", up="_Enter", target_id="{target_id}")
                    + "4 2 _Enter",
                    4,
                    2,
                ),
                "cylinder_isocurve": run_scenario(
                    "cylinder-isocurve",
                    "cylinder",
                    base.format(
                        mode="_Isocurve", up="_Enter", target_id="{target_id}"
                    )
                    + "4 2 _Enter",
                    4,
                    2,
                ),
                "warped_isocurve": run_scenario(
                    "warped-isocurve",
                    "warped",
                    base.format(
                        mode="_Isocurve", up="_Enter", target_id="{target_id}"
                    )
                    + "4 3 _Enter",
                    4,
                    3,
                ),
                "single": run_scenario(
                    "single",
                    "warped",
                    base.format(mode="_UV", up="_Enter", target_id="{target_id}")
                    + "1 1 _Enter",
                    1,
                    1,
                ),
                "custom_up": run_scenario(
                    "custom-up",
                    "bilinear",
                    base.format(mode="_UV", up="1,3,3", target_id="{target_id}")
                    + "1 1 _Enter",
                    1,
                    1,
                ),
            }
            timing_surface = create_surface("cylinder")
            _unused, elapsed = _measure(
                iterations, lambda: timing_surface.FrameAt(0.37, 0.62)
            )
            return value, elapsed
        finally:
            document.Objects.UnselectAll()
            objects = fixture_objects()
            for group_index in sorted(fixture_group_indices, reverse=True):
                if not document.Groups.IsDeleted(group_index):
                    document.Groups.Delete(group_index)
            for item in objects:
                document.Objects.Delete(item.Id, True)
            for target_id in target_ids:
                document.Objects.Delete(target_id, True)
    if kind == "document_orient_cycle":
        document = Rhino.RhinoDoc.ActiveDoc
        suffix = str(System.Guid.NewGuid())
        name_prefix = "Viboceros Orient " + suffix + " "
        fixture_group_indices = set()

        def fixture_objects():
            objects = []
            for rhino_object in document.Objects:
                name = rhino_object.Attributes.Name
                if name is not None and name.startswith(name_prefix):
                    objects.append(rhino_object)
            return objects

        def line_record(rhino_object):
            geometry = rhino_object.Geometry
            return {
                "end": _xyz(geometry.PointAtEnd),
                "name": rhino_object.Attributes.Name[len(name_prefix):],
                "selected": rhino_object.IsSelected(False) > 0,
                "start": _xyz(geometry.PointAtStart),
            }

        def record_key(record):
            coordinates = record["start"] + record["end"]
            rounded = [round(value, 12) for value in coordinates]
            return tuple(rounded + [record["name"]])

        def scenario_objects(label):
            prefix = name_prefix + label + " "
            objects = [
                item
                for item in fixture_objects()
                if item.Attributes.Name.startswith(prefix)
            ]
            objects.sort(key=lambda item: record_key(line_record(item)))
            return objects

        def scenario_groups(objects):
            fixture_ids = set(item.Id for item in objects)
            groups = []
            for group_index in range(document.Groups.Count):
                if document.Groups.IsDeleted(group_index):
                    continue
                members = document.Groups.GroupMembers(group_index)
                if members is None:
                    continue
                records = [
                    line_record(member)
                    for member in members
                    if member.Id in fixture_ids
                ]
                if records:
                    fixture_group_indices.add(group_index)
                    records.sort(key=record_key)
                    groups.append(records)
            groups.sort(
                key=lambda group: tuple(record_key(record) for record in group)
            )
            return groups

        def run_scenario(label, command, expected_object_count):
            _record_progress("document_orient_cycle: %s start" % label)
            source_ids = []
            origin = Rhino.Geometry.Point3d(1.0, 2.0, 3.0)
            for axis, offset in (
                ("x", Rhino.Geometry.Vector3d(1.0, 0.0, 0.0)),
                ("y", Rhino.Geometry.Vector3d(0.0, 1.0, 0.0)),
                ("z", Rhino.Geometry.Vector3d(0.0, 0.0, 1.0)),
            ):
                attributes = Rhino.DocObjects.ObjectAttributes()
                attributes.Name = name_prefix + label + " " + axis
                source_id = document.Objects.AddLine(origin, origin + offset, attributes)
                if source_id == System.Guid.Empty:
                    raise ValueError("could not add orient fixture line")
                source_ids.append(source_id)
            group_index = document.Groups.Add(
                "Viboceros Orient Group " + suffix + " " + label,
                source_ids,
            )
            if group_index < 0:
                raise ValueError("could not group orient fixture lines")
            fixture_group_indices.add(group_index)
            document.Objects.UnselectAll()
            for source_id in source_ids:
                if not document.Objects.Select(source_id):
                    raise ValueError("could not select orient fixture line")
            command_succeeded = Rhino.RhinoApp.RunScript(command, False)
            _record_progress("document_orient_cycle: %s command complete" % label)
            objects = scenario_objects(label)
            if len(objects) != expected_object_count:
                history = Rhino.RhinoApp.CommandHistoryWindowText
                raise ValueError(
                    "orient macro %r returned %r and left %d fixture objects; "
                    "history tail: %s"
                    % (
                        command,
                        command_succeeded,
                        len(objects),
                        history[-2000:],
                    )
                )
            records = [line_record(item) for item in objects]
            records.sort(key=record_key)
            return {
                "command_succeeded": bool(command_succeeded),
                "groups": scenario_groups(objects),
                "objects": records,
                "originals_selected": [
                    index
                    for index, source_id in enumerate(source_ids)
                    if document.Objects.FindId(source_id).IsSelected(False) > 0
                ],
            }

        try:
            orient = "_-Orient 1,2,3 3,2,3 "
            targets = " 10,-1,4 10,5,4 _Enter"
            orient3 = "_-Orient3Pt 1,2,3 3,2,3 1,3,4 "
            targets3 = " 10,-1,4 10,5,4 8,-1,8 _Enter"
            value = {
                "orient_default": run_scenario(
                    "orient-default", orient + targets, 3
                ),
                "orient_copy_no": run_scenario(
                    "orient-copy-no",
                    orient + "_Copy=_Yes _Scale=_No" + targets + " _Enter",
                    6,
                ),
                "orient_copy_1d": run_scenario(
                    "orient-copy-1d",
                    orient + "_Copy=_Yes _Scale=_1D" + targets + " _Enter",
                    6,
                ),
                "orient_copy_3d": run_scenario(
                    "orient-copy-3d",
                    orient + "_Copy=_Yes _Scale=_3D" + targets + " _Enter",
                    6,
                ),
                "orient_spatial": run_scenario(
                    "orient-spatial",
                    "_-Orient 1,2,3 2,4,6 _Copy=_Yes _Scale=_No "
                    "-5,4,2 -7,8,3 _Enter _Enter",
                    6,
                ),
                "orient3_default": run_scenario(
                    "orient3-default", orient3 + targets3, 3
                ),
                "orient3_copy_scale": run_scenario(
                    "orient3-copy-scale",
                    orient3 + "_Copy=_Yes _Scale=_Yes" + targets3 + " _Enter",
                    6,
                ),
            }
            source_direction = Rhino.Geometry.Vector3d(1.0, 2.0, 3.0)
            target_direction = Rhino.Geometry.Vector3d(-2.0, 4.0, 1.0)
            origin = Rhino.Geometry.Point3d(1.0, 2.0, 3.0)
            _unused, elapsed = _measure(
                iterations,
                lambda: Rhino.Geometry.Transform.Rotation(
                    source_direction, target_direction, origin
                ),
            )
            return value, elapsed
        finally:
            document.Objects.UnselectAll()
            objects = fixture_objects()
            for group_index in sorted(fixture_group_indices, reverse=True):
                document.Groups.Delete(group_index)
            for item in objects:
                document.Objects.Delete(item.Id, True)
    if kind == "document_curve_array_cycle":
        document = Rhino.RhinoDoc.ActiveDoc
        suffix = str(System.Guid.NewGuid())
        name_prefix = "Viboceros Curve Array " + suffix + " "
        path_ids = []
        fixture_group_indices = set()

        def fixture_objects():
            objects = []
            for rhino_object in document.Objects:
                name = rhino_object.Attributes.Name
                if name is not None and name.startswith(name_prefix):
                    objects.append(rhino_object)
            return objects

        def fixture_xyz(value):
            coordinates = [round(float(component), 6) for component in value]
            return [0.0 if component == 0.0 else component for component in coordinates]

        def line_record(rhino_object):
            geometry = rhino_object.Geometry
            return {
                "end": fixture_xyz(geometry.PointAtEnd),
                "name": rhino_object.Attributes.Name[len(name_prefix):],
                "selected": rhino_object.IsSelected(False) > 0,
                "start": fixture_xyz(geometry.PointAtStart),
            }

        def record_key(record):
            coordinates = record["start"] + record["end"]
            rounded = [round(value, 12) for value in coordinates]
            return tuple(rounded + [record["name"]])

        def scenario_objects(label):
            prefix = name_prefix + label + " "
            objects = [
                item
                for item in fixture_objects()
                if item.Attributes.Name.startswith(prefix)
            ]
            objects.sort(key=lambda item: record_key(line_record(item)))
            return objects

        def scenario_groups(objects):
            fixture_ids = set(item.Id for item in objects)
            groups = []
            for group_index in range(document.Groups.Count):
                if document.Groups.IsDeleted(group_index):
                    continue
                members = document.Groups.GroupMembers(group_index)
                if members is None:
                    continue
                records = [
                    line_record(member)
                    for member in members
                    if member.Id in fixture_ids
                ]
                if records:
                    fixture_group_indices.add(group_index)
                    records.sort(key=record_key)
                    groups.append(records)
            groups.sort(
                key=lambda group: tuple(record_key(record) for record in group)
            )
            return groups

        def add_path(path_kind):
            if path_kind == "line":
                return document.Objects.AddLine(
                    Rhino.Geometry.Point3d(0.0, 0.0, 0.0),
                    Rhino.Geometry.Point3d(10.0, 0.0, 0.0),
                )
            if path_kind == "nurbs":
                curve = Rhino.Geometry.NurbsCurve(3, True, 4, 5)
                _set_curve_controls(
                    curve,
                    [
                        {"point": [0.0, 0.0, 0.0]},
                        {"point": [2.0, 0.0, 3.0]},
                        {"point": [4.0, 3.0, -1.0]},
                        {"point": [7.0, 5.0, 4.0]},
                        {"point": [10.0, 8.0, 6.0]},
                    ],
                )
                _set_knots(
                    curve.Knots,
                    [0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0, 2.0],
                    "curve-array NURBS knot",
                )
                if not curve.IsValid:
                    raise ValueError("curve-array NURBS path is invalid")
                return document.Objects.AddCurve(curve)
            return document.Objects.AddArc(
                Rhino.Geometry.Arc(
                    Rhino.Geometry.Point3d(5.0, 0.0, 0.0),
                    Rhino.Geometry.Point3d(0.0, 3.0, 4.0),
                    Rhino.Geometry.Point3d(-5.0, 0.0, 0.0),
                )
            )

        def run_scenario(
            label,
            path_kind,
            source_anchor,
            command_template,
            expected_instance_count,
        ):
            _record_progress("document_curve_array_cycle: %s start" % label)
            anchor = _point(source_anchor)
            source_ids = []
            # Keep the spatial endpoints away from half-micro rounding
            # boundaries while retaining a longer-than-unit frame witness.
            source_axis_length = 1.25 if path_kind == "nurbs" else 1.0
            for axis, offset in (
                ("x", Rhino.Geometry.Vector3d(source_axis_length, 0.0, 0.0)),
                ("y", Rhino.Geometry.Vector3d(0.0, source_axis_length, 0.0)),
                ("z", Rhino.Geometry.Vector3d(0.0, 0.0, source_axis_length)),
            ):
                attributes = Rhino.DocObjects.ObjectAttributes()
                attributes.Name = name_prefix + label + " " + axis
                source_id = document.Objects.AddLine(
                    anchor, anchor + offset, attributes
                )
                if source_id == System.Guid.Empty:
                    raise ValueError("could not add curve-array fixture line")
                source_ids.append(source_id)
            group_index = document.Groups.Add(
                "Viboceros Curve Array Group " + suffix + " " + label,
                source_ids,
            )
            if group_index < 0:
                raise ValueError("could not group curve-array fixture objects")
            fixture_group_indices.add(group_index)
            path_id = add_path(path_kind)
            if path_id == System.Guid.Empty:
                raise ValueError("could not add curve-array fixture path")
            path_ids.append(path_id)
            document.Objects.UnselectAll()
            for source_id in source_ids:
                if not document.Objects.Select(source_id):
                    raise ValueError("could not select curve-array fixture object")

            command = command_template.replace("{path_id}", str(path_id))
            command_succeeded = Rhino.RhinoApp.RunScript(command, False)
            _record_progress(
                "document_curve_array_cycle: %s command complete" % label
            )
            objects = scenario_objects(label)
            expected_count = len(source_ids) * expected_instance_count
            if len(objects) != expected_count:
                history = Rhino.RhinoApp.CommandHistoryWindowText
                raise ValueError(
                    "ArrayCrv macro %r returned %r and left %d fixture objects; "
                    "history tail: %s"
                    % (
                        command,
                        command_succeeded,
                        len(objects),
                        history[-2000:],
                    )
                )
            records = [line_record(item) for item in objects]
            records.sort(key=record_key)
            _record_progress(
                "document_curve_array_cycle: %s objects captured" % label
            )
            groups = scenario_groups(objects)
            _record_progress(
                "document_curve_array_cycle: %s groups captured" % label
            )
            originals_selected = []
            for index, source_id in enumerate(source_ids):
                source_object = document.Objects.FindId(source_id)
                if (
                    source_object is not None
                    and source_object.IsSelected(False) > 0
                ):
                    originals_selected.append(index)
            path_object = document.Objects.FindId(path_id)
            _record_progress(
                "document_curve_array_cycle: %s selection captured" % label
            )
            result = {
                "command_succeeded": bool(command_succeeded),
                "groups": groups,
                "objects": records,
                "originals_selected": originals_selected,
                "path_selected": (
                    path_object is not None and path_object.IsSelected(False) > 0
                ),
            }
            return result

        try:
            value = {
                "base_point": run_scenario(
                    "base-point",
                    "line",
                    [20.0, 0.0, 0.0],
                    "_-ArrayCrv _Basepoint 20,0,0 "
                    "'_-SelID {path_id} _Orientation _NoRotation 4",
                    5,
                ),
                "freeform": run_scenario(
                    "freeform",
                    "tilted-arc",
                    [5.0, 0.0, 0.0],
                    "_-ArrayCrv '_-SelID {path_id} "
                    "_Orientation _Freeform 4",
                    4,
                ),
                "freeform_nurbs": run_scenario(
                    "freeform-nurbs",
                    "nurbs",
                    [0.0, 0.0, 0.0],
                    "_-ArrayCrv '_-SelID {path_id} "
                    "_Orientation _Freeform 5",
                    5,
                ),
                "no_rotation_distance": run_scenario(
                    "no-rotation-distance",
                    "line",
                    [0.0, 0.0, 0.0],
                    "_-ArrayCrv '_-SelID {path_id} "
                    "_Orientation _NoRotation _Distance 3 _Enter",
                    4,
                ),
                "no_rotation_items": run_scenario(
                    "no-rotation-items",
                    "line",
                    [0.0, 0.0, 0.0],
                    "_-ArrayCrv '_-SelID {path_id} "
                    "_Orientation _NoRotation 4",
                    4,
                ),
                "roadlike": run_scenario(
                    "roadlike",
                    "tilted-arc",
                    [5.0, 0.0, 0.0],
                    "_-ArrayCrv '_-SelID {path_id} "
                    "_Orientation _Roadlike 4",
                    4,
                ),
                "stairlike": run_scenario(
                    "stairlike",
                    "tilted-arc",
                    [5.0, 0.0, 0.0],
                    "_-ArrayCrv '_-SelID {path_id} "
                    "_Orientation _Stairlike 4",
                    4,
                ),
            }
            timing_curve = Rhino.Geometry.LineCurve(
                Rhino.Geometry.Point3d(0.0, 0.0, 0.0),
                Rhino.Geometry.Point3d(10.0, 0.0, 0.0),
            )
            _unused, elapsed = _measure(
                iterations, lambda: timing_curve.DivideByCount(3, True)
            )
            _record_progress("document_curve_array_cycle: timing complete")
            return value, elapsed
        finally:
            _record_progress("document_curve_array_cycle: cleanup start")
            try:
                document.Objects.UnselectAll()
            except Exception:
                pass
            try:
                objects = fixture_objects()
                for group_index in sorted(fixture_group_indices, reverse=True):
                    try:
                        document.Groups.Delete(group_index)
                    except Exception:
                        pass
                for item in objects:
                    try:
                        document.Objects.Delete(item.Id, True)
                    except Exception:
                        pass
            except Exception:
                pass
            for path_id in path_ids:
                try:
                    document.Objects.Delete(path_id, True)
                except Exception:
                    pass
    if kind == "document_rectangular_array_cycle":
        document = Rhino.RhinoDoc.ActiveDoc
        suffix = str(System.Guid.NewGuid())
        name_prefix = "Viboceros Rectangular Array " + suffix + " "
        fixture_group_indices = set()

        def fixture_objects():
            objects = []
            for rhino_object in document.Objects:
                name = rhino_object.Attributes.Name
                if name is not None and name.startswith(name_prefix):
                    objects.append(rhino_object)
            return objects

        def point_key(item):
            return (
                round(float(item.Geometry.Location.X), 12),
                round(float(item.Geometry.Location.Y), 12),
                round(float(item.Geometry.Location.Z), 12),
                item.Attributes.Name,
            )

        def scenario_objects(label):
            prefix = name_prefix + label + " "
            objects = [
                item
                for item in fixture_objects()
                if item.Attributes.Name.startswith(prefix)
            ]
            objects.sort(key=point_key)
            return objects

        def locations(objects):
            return [_xyz(item.Geometry.Location) for item in objects]

        def selected_locations(objects):
            return locations([item for item in objects if item.IsSelected(False) > 0])

        def scenario_groups(objects):
            fixture_ids = set(item.Id for item in objects)
            groups = []
            for group_index in range(document.Groups.Count):
                if document.Groups.IsDeleted(group_index):
                    continue
                members = document.Groups.GroupMembers(group_index)
                if members is None:
                    continue
                fixture_members = [
                    member for member in members if member.Id in fixture_ids
                ]
                if fixture_members:
                    fixture_group_indices.add(group_index)
                    fixture_members.sort(key=point_key)
                    groups.append(locations(fixture_members))
            groups.sort(
                key=lambda group: tuple(
                    tuple(round(value, 12) for value in point) for point in group
                )
            )
            return groups

        def run_scenario(label, command, x_count, y_count, z_count):
            _record_progress("document_rectangular_array_cycle: %s start" % label)
            original_ids = []
            for index, coordinates in enumerate(
                ((1.0, 2.0, 3.0), (4.0, 2.0, 3.0))
            ):
                attributes = Rhino.DocObjects.ObjectAttributes()
                attributes.Name = name_prefix + label + " " + str(index)
                object_id = document.Objects.AddPoint(
                    Rhino.Geometry.Point3d(*coordinates), attributes
                )
                if object_id == System.Guid.Empty:
                    raise ValueError("could not add rectangular-array fixture point")
                original_ids.append(object_id)
            original_group_index = document.Groups.Add(
                "Viboceros Rectangular Array Group " + suffix + " " + label,
                original_ids,
            )
            if original_group_index < 0:
                raise ValueError("could not group rectangular-array fixture objects")
            fixture_group_indices.add(original_group_index)
            document.Objects.UnselectAll()
            for object_id in original_ids:
                if not document.Objects.Select(object_id):
                    raise ValueError("could not select rectangular-array fixture object")

            command_succeeded = Rhino.RhinoApp.RunScript(command, False)
            _record_progress(
                "document_rectangular_array_cycle: %s command complete" % label
            )
            array_objects = scenario_objects(label)
            expected_count = 2 * x_count * y_count * z_count
            if len(array_objects) != expected_count:
                history = Rhino.RhinoApp.CommandHistoryWindowText
                raise ValueError(
                    "Array macro %r returned %r and left %d fixture objects; "
                    "history tail: %s"
                    % (command, command_succeeded, len(array_objects), history[-2000:])
                )
            return {
                "command_succeeded": bool(command_succeeded),
                "groups_after_array": scenario_groups(array_objects),
                "locations_after_array": locations(array_objects),
                "names_after_array": [
                    item.Attributes.Name[len(name_prefix):] for item in array_objects
                ],
                "originals_selected_after_array": [
                    index
                    for index, object_id in enumerate(original_ids)
                    if document.Objects.FindId(object_id).IsSelected(False) > 0
                ],
                "selected_after_array": selected_locations(array_objects),
            }

        try:
            value = {
                "fill": run_scenario(
                    "fill",
                    "_-Array _Mode=_Fill 3 2 1 10 -6 _Enter",
                    3,
                    2,
                    1,
                ),
                "unit_cell": run_scenario(
                    "unit-cell",
                    "_-Array _Mode=_UnitCell 3 2 2 2 -1 4 _Enter",
                    3,
                    2,
                    2,
                ),
            }

            source_points = [
                Rhino.Geometry.Point3d(1.0, 2.0, 3.0),
                Rhino.Geometry.Point3d(4.0, 2.0, 3.0),
            ]

            def compute_array_points():
                return [
                    point
                    + Rhino.Geometry.Vector3d(
                        2.0 * x_index, -1.0 * y_index, 4.0 * z_index
                    )
                    for z_index in range(2)
                    for y_index in range(2)
                    for x_index in range(3)
                    if x_index != 0 or y_index != 0 or z_index != 0
                    for point in source_points
                ]

            _unused, elapsed = _measure(iterations, compute_array_points)
            return value, elapsed
        finally:
            document.Objects.UnselectAll()
            objects = fixture_objects()
            for group_index in sorted(fixture_group_indices, reverse=True):
                document.Groups.Delete(group_index)
            for item in objects:
                document.Objects.Delete(item.Id, True)
    if kind == "document_polar_array_cycle":
        document = Rhino.RhinoDoc.ActiveDoc
        suffix = str(System.Guid.NewGuid())
        name_prefix = "Viboceros Polar Array " + suffix + " "
        fixture_group_indices = set()

        def fixture_objects():
            objects = []
            for rhino_object in document.Objects:
                name = rhino_object.Attributes.Name
                if name is not None and name.startswith(name_prefix):
                    objects.append(rhino_object)
            return objects

        def line_record(rhino_object):
            geometry = rhino_object.Geometry
            return {
                "end": _xyz(geometry.PointAtEnd),
                "name": rhino_object.Attributes.Name[len(name_prefix):],
                "selected": rhino_object.IsSelected(False) > 0,
                "start": _xyz(geometry.PointAtStart),
            }

        def record_key(record):
            coordinates = record["start"] + record["end"]
            return tuple([round(value, 12) for value in coordinates] + [record["name"]])

        def scenario_objects(label):
            prefix = name_prefix + label + " "
            objects = [
                item
                for item in fixture_objects()
                if item.Attributes.Name.startswith(prefix)
            ]
            objects.sort(key=lambda item: record_key(line_record(item)))
            return objects

        def scenario_groups(objects):
            fixture_ids = set(item.Id for item in objects)
            groups = []
            for group_index in range(document.Groups.Count):
                if document.Groups.IsDeleted(group_index):
                    continue
                members = document.Groups.GroupMembers(group_index)
                if members is None:
                    continue
                records = [
                    line_record(member)
                    for member in members
                    if member.Id in fixture_ids
                ]
                if records:
                    fixture_group_indices.add(group_index)
                    records.sort(key=record_key)
                    groups.append(records)
            groups.sort(key=lambda group: tuple(record_key(record) for record in group))
            return groups

        def run_scenario(
            label,
            item_count,
            angle_degrees,
            rotate,
            z_offset=None,
        ):
            _record_progress("document_polar_array_cycle: %s start" % label)
            original_ids = []
            source_lines = (
                ((2.0, 0.0, 0.0), (4.0, 1.0, 0.0)),
                ((1.0, -1.0, 2.0), (2.0, -0.5, 3.0)),
            )
            for index, endpoints in enumerate(source_lines):
                attributes = Rhino.DocObjects.ObjectAttributes()
                attributes.Name = name_prefix + label + " " + str(index)
                line = Rhino.Geometry.Line(_point(endpoints[0]), _point(endpoints[1]))
                object_id = document.Objects.AddLine(line, attributes)
                if object_id == System.Guid.Empty:
                    raise ValueError("could not add polar-array fixture line")
                original_ids.append(object_id)
            group_index = document.Groups.Add(
                "Viboceros Polar Array Group " + suffix + " " + label,
                original_ids,
            )
            if group_index < 0:
                raise ValueError("could not group polar-array fixture objects")
            fixture_group_indices.add(group_index)
            document.Objects.UnselectAll()
            for object_id in original_ids:
                if not document.Objects.Select(object_id):
                    raise ValueError("could not select polar-array fixture object")

            options = "_Rotate=_%s" % ("Yes" if rotate else "No")
            options += " _ZOffset %.17g" % (0.0 if z_offset is None else z_offset)
            command = "_-ArrayPolar %s %d %s %.17g _Enter" % (
                "0,0,0",
                item_count,
                options,
                angle_degrees,
            )
            command_succeeded = Rhino.RhinoApp.RunScript(command, False)
            _record_progress("document_polar_array_cycle: %s command complete" % label)
            objects = scenario_objects(label)
            expected_count = len(source_lines) * item_count
            if len(objects) != expected_count:
                history = Rhino.RhinoApp.CommandHistoryWindowText
                raise ValueError(
                    "ArrayPolar macro %r returned %r and left %d fixture objects; "
                    "history tail: %s"
                    % (command, command_succeeded, len(objects), history[-2000:])
                )
            records = [line_record(item) for item in objects]
            records.sort(key=record_key)
            return {
                "command_succeeded": bool(command_succeeded),
                "groups": scenario_groups(objects),
                "objects": records,
                "originals_selected": [
                    index
                    for index, object_id in enumerate(original_ids)
                    if document.Objects.FindId(object_id).IsSelected(False) > 0
                ],
            }

        try:
            value = {
                "full_rotate_yes": run_scenario("full", 4, 360.0, True),
                "negative_full_rotate_yes": run_scenario(
                    "negative-full", 4, -360.0, True
                ),
                "multi_turn_z_offset_rotate_yes": run_scenario(
                    "multi-turn", 4, 720.0, True, z_offset=2.0
                ),
                "partial_rotate_no": run_scenario("partial-no", 4, 180.0, False),
                "partial_rotate_yes": run_scenario("partial-yes", 4, 180.0, True),
                "z_offset_rotate_yes": run_scenario(
                    "z-offset", 4, 180.0, True, z_offset=2.0
                ),
            }
            source_points = [
                Rhino.Geometry.Point3d(2.0, 0.0, 0.0),
                Rhino.Geometry.Point3d(4.0, 1.0, 0.0),
            ]
            transforms = [
                Rhino.Geometry.Transform.Rotation(
                    math.radians(90.0 * copy_index),
                    Rhino.Geometry.Vector3d.ZAxis,
                    Rhino.Geometry.Point3d.Origin,
                )
                for copy_index in range(1, 4)
            ]

            def compute_array_points():
                result = []
                for transform in transforms:
                    for source in source_points:
                        point = Rhino.Geometry.Point3d(source)
                        point.Transform(transform)
                        result.append(point)
                return result

            _unused, elapsed = _measure(iterations, compute_array_points)
            return value, elapsed
        finally:
            document.Objects.UnselectAll()
            objects = fixture_objects()
            for group_index in sorted(fixture_group_indices, reverse=True):
                document.Groups.Delete(group_index)
            for item in objects:
                document.Objects.Delete(item.Id, True)
    if kind == "document_linear_array_cycle":
        document = Rhino.RhinoDoc.ActiveDoc
        suffix = str(System.Guid.NewGuid())
        name_prefix = "Viboceros Linear Array " + suffix + " "
        original_ids = []
        fixture_group_indices = set()

        def fixture_objects():
            objects = []
            for rhino_object in document.Objects:
                name = rhino_object.Attributes.Name
                if name is not None and name.startswith(name_prefix):
                    objects.append(rhino_object)
            objects.sort(
                key=lambda item: (
                    float(item.Geometry.Location.X),
                    float(item.Geometry.Location.Y),
                    float(item.Geometry.Location.Z),
                    item.Attributes.Name,
                )
            )
            return objects

        def locations(objects):
            return [_xyz(item.Geometry.Location) for item in objects]

        def selected_locations(objects):
            return locations([item for item in objects if item.IsSelected(False) > 0])

        def fixture_groups(objects):
            fixture_ids = set(item.Id for item in objects)
            groups = []
            for group_index in range(document.Groups.Count):
                if document.Groups.IsDeleted(group_index):
                    continue
                members = document.Groups.GroupMembers(group_index)
                if members is None:
                    continue
                fixture_members = [
                    member for member in members if member.Id in fixture_ids
                ]
                if fixture_members:
                    fixture_group_indices.add(group_index)
                    fixture_members.sort(
                        key=lambda item: (
                            float(item.Geometry.Location.X),
                            float(item.Geometry.Location.Y),
                            float(item.Geometry.Location.Z),
                        )
                    )
                    groups.append(locations(fixture_members))
            groups.sort(key=lambda group: tuple(tuple(point) for point in group))
            return groups

        try:
            for index, coordinates in enumerate(((1.0, 2.0, 3.0), (4.0, 2.0, 3.0))):
                attributes = Rhino.DocObjects.ObjectAttributes()
                attributes.Name = name_prefix + str(index)
                object_id = document.Objects.AddPoint(
                    Rhino.Geometry.Point3d(*coordinates), attributes
                )
                if object_id == System.Guid.Empty:
                    raise ValueError("could not add linear-array fixture point")
                original_ids.append(object_id)
            original_group_index = document.Groups.Add(
                "Viboceros Linear Array Group " + suffix, original_ids
            )
            if original_group_index < 0:
                raise ValueError("could not group linear-array fixture objects")
            fixture_group_indices.add(original_group_index)
            document.Objects.UnselectAll()
            for object_id in original_ids:
                if not document.Objects.Select(object_id):
                    raise ValueError("could not select linear-array fixture object")

            command_succeeded = Rhino.RhinoApp.RunScript(
                "_-ArrayLinear 4 0,0,0 2,-1,3 _Enter", False
            )
            array_objects = fixture_objects()
            if len(array_objects) != 8:
                raise ValueError(
                    "ArrayLinear returned %r and left %d fixture objects"
                    % (command_succeeded, len(array_objects))
                )
            value = {
                "command_succeeded": bool(command_succeeded),
                "groups_after_array": fixture_groups(array_objects),
                "locations_after_array": locations(array_objects),
                "names_after_array": [
                    item.Attributes.Name[len(name_prefix):] for item in array_objects
                ],
                "originals_selected_after_array": [
                    index
                    for index, object_id in enumerate(original_ids)
                    if document.Objects.FindId(object_id).IsSelected(False) > 0
                ],
                "selected_after_array": selected_locations(array_objects),
            }

            source_points = [
                Rhino.Geometry.Point3d(1.0, 2.0, 3.0),
                Rhino.Geometry.Point3d(4.0, 2.0, 3.0),
            ]
            spacing = Rhino.Geometry.Vector3d(2.0, -1.0, 3.0)

            def compute_array_points():
                return [
                    point + spacing * copy_index
                    for copy_index in range(1, 4)
                    for point in source_points
                ]

            _unused, elapsed = _measure(iterations, compute_array_points)
            return value, elapsed
        finally:
            document.Objects.UnselectAll()
            objects = fixture_objects()
            for group_index in sorted(fixture_group_indices, reverse=True):
                document.Groups.Delete(group_index)
            for item in objects:
                document.Objects.Delete(item.Id, True)
    if kind == "three_dm_group_round_trip":
        _record_progress("three_dm_group_round_trip: start")
        path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "groups.3dm")
        model = Rhino.FileIO.File3dm()
        _record_progress("three_dm_group_round_trip: model created")
        decoded = None
        try:
            layer_index = model.Layers.AddDefaultLayer(
                "Default", System.Drawing.Color.Black
            )
            _record_progress("three_dm_group_round_trip: layer added")
            if layer_index < 0:
                raise ValueError("could not add file group fixture layer")
            group_names = ["Assembly α", "Inspection", "Empty Group"]
            group_indices = []
            for name in group_names:
                group_index = model.AllGroups.AddGroup()
                group = model.AllGroups.FindIndex(group_index)
                if group_index < 0 or group is None:
                    raise ValueError("could not add file group fixture group")
                group.Name = name
                group_indices.append(group_index)
                _record_progress("three_dm_group_round_trip: group added")

            memberships = [[0], [0, 1], [1], []]
            object_colors = [
                [12, 34, 56],
                [23, 45, 67],
                [34, 56, 78],
                [45, 67, 89],
            ]
            color_sources = [
                Rhino.DocObjects.ObjectColorSource.ColorFromObject,
                Rhino.DocObjects.ObjectColorSource.ColorFromLayer,
                Rhino.DocObjects.ObjectColorSource.ColorFromMaterial,
                Rhino.DocObjects.ObjectColorSource.ColorFromParent,
            ]
            for object_index, membership in enumerate(memberships):
                attributes = Rhino.DocObjects.ObjectAttributes()
                attributes.LayerIndex = layer_index
                attributes.Name = "P%d" % object_index
                color = object_colors[object_index]
                attributes.ObjectColor = System.Drawing.Color.FromArgb(
                    color[0], color[1], color[2]
                )
                attributes.ColorSource = color_sources[object_index]
                for group_position in membership:
                    attributes.AddToGroup(group_indices[group_position])
                object_id = model.Objects.AddPoint(
                    Rhino.Geometry.Point3d(float(object_index), 0.0, 0.0),
                    attributes,
                )
                if object_id == System.Guid.Empty:
                    raise ValueError("could not add file group fixture point")
                _record_progress("three_dm_group_round_trip: object added")
            if not model.Write(path, 8):
                raise ValueError("could not write file group fixture")
            _record_progress("three_dm_group_round_trip: model written")

            decoded = Rhino.FileIO.File3dm.Read(path)
            _record_progress("three_dm_group_round_trip: model read")
            if decoded is None:
                raise ValueError("could not read file group fixture")
            decoded_objects = sorted(
                list(decoded.Objects), key=lambda item: item.Attributes.Name
            )
            _record_progress("three_dm_group_round_trip: objects decoded")
            decoded_groups = sorted(
                list(decoded.AllGroups), key=lambda item: int(item.Index)
            )
            _record_progress("three_dm_group_round_trip: groups decoded")
            decoded_group_names = [group.Name for group in decoded_groups]
            group_names_by_index = {
                int(group.Index): group.Name for group in decoded_groups
            }
            _record_progress("three_dm_group_round_trip: names decoded")
            group_positions_by_index = {
                int(group.Index): position
                for position, group in enumerate(decoded_groups)
            }
            # File3dmGroupTable.GroupMembers throws on this Rhino/Wine host;
            # invert the persisted ObjectAttributes lists instead.
            group_members = [[] for _group in decoded_groups]
            object_groups = []
            decoded_object_colors = []
            decoded_color_sources = []
            for object_position, item in enumerate(decoded_objects):
                _record_progress(
                    "three_dm_group_round_trip: reading object %s groups"
                    % item.Attributes.Name
                )
                indices = item.Attributes.GetGroupList()
                indices = [] if indices is None else [int(index) for index in indices]
                object_groups.append(
                    [group_names_by_index[index] for index in indices]
                )
                for index in indices:
                    group_members[group_positions_by_index[index]].append(object_position)
                color = item.Attributes.ObjectColor
                decoded_object_colors.append(
                    [int(color.R), int(color.G), int(color.B)]
                )
                source = item.Attributes.ColorSource
                if source == Rhino.DocObjects.ObjectColorSource.ColorFromLayer:
                    decoded_color_sources.append("layer")
                elif source == Rhino.DocObjects.ObjectColorSource.ColorFromObject:
                    decoded_color_sources.append("object")
                elif source == Rhino.DocObjects.ObjectColorSource.ColorFromMaterial:
                    decoded_color_sources.append("material")
                elif source == Rhino.DocObjects.ObjectColorSource.ColorFromParent:
                    decoded_color_sources.append("parent")
                else:
                    raise ValueError("file group fixture has an unknown color source")
            _record_progress("three_dm_group_round_trip: memberships decoded")
            value = {
                "color_sources": decoded_color_sources,
                "group_members": group_members,
                "group_names": decoded_group_names,
                "object_colors": decoded_object_colors,
                "object_groups": object_groups,
                "unsupported_object_count": 0,
            }
            _unused, elapsed = _measure(
                iterations,
                lambda: sum(
                    int(item.Attributes.GroupCount) for item in decoded_objects
                ),
            )
            _record_progress("three_dm_group_round_trip: complete")
            return value, elapsed
        finally:
            if decoded is not None:
                decoded.Dispose()
            model.Dispose()
            if os.path.exists(path):
                os.remove(path)
    if kind == "document_point_cloud_cycle":
        document = Rhino.RhinoDoc.ActiveDoc
        suffix = str(System.Guid.NewGuid()).replace("-", "")
        default_layer_index = int(document.Layers.CurrentLayerIndex)
        layer_indices = []
        source_ids = []

        def all_object_ids():
            settings = Rhino.DocObjects.ObjectEnumeratorSettings()
            settings.NormalObjects = True
            settings.LockedObjects = True
            settings.HiddenObjects = True
            return set(obj.Id for obj in document.Objects.GetObjectList(settings))

        def layer_label(layer_index):
            if layer_index == default_layer_index:
                return "Current"
            if layer_index == layer_indices[0]:
                return "A"
            if layer_index == layer_indices[1]:
                return "B"
            return "Unexpected"

        def selected_labels():
            labels = []
            for label, object_id in zip(
                ("line", "mesh", "cloud", "point"), source_ids
            ):
                rhino_object = document.Objects.FindId(object_id)
                if (
                    rhino_object is not None
                    and rhino_object.IsSelected(False) != 0
                ):
                    labels.append(label)
            return labels

        def describe(ids):
            values = []
            for object_id in ids:
                rhino_object = document.Objects.FindId(object_id)
                geometry = rhino_object.Geometry
                if isinstance(geometry, Rhino.Geometry.PointCloud):
                    geometry_type = "point_cloud"
                    points = [_xyz(point) for point in geometry.GetPoints()]
                elif isinstance(geometry, Rhino.Geometry.Point):
                    geometry_type = "point"
                    points = [_xyz(geometry.Location)]
                else:
                    geometry_type = str(rhino_object.ObjectType)
                    points = []
                values.append(
                    {
                        "layer": layer_label(
                            int(rhino_object.Attributes.LayerIndex)
                        ),
                        "name": rhino_object.Attributes.Name,
                        "points": points,
                        "selected": rhino_object.IsSelected(False) != 0,
                        "type": geometry_type,
                    }
                )
            values.sort(
                key=lambda value: (
                    value["layer"],
                    value["type"],
                    value["points"],
                )
            )
            return values

        def delete_objects(ids):
            document.Objects.UnselectAll()
            for object_id in ids:
                if not document.Objects.Delete(object_id, True):
                    raise ValueError("could not delete point-cloud cycle output")

        def run_extract(ids, output, output_layer):
            document.Objects.UnselectAll()
            before = all_object_ids()
            command = (
                "_-ExtractPt _OutputLayer=%s _Output=%s %s _Enter"
                % (
                    output_layer,
                    output,
                    " ".join("_SelID %s" % object_id for object_id in ids),
                )
            )
            succeeded = bool(
                Rhino.RhinoApp.RunScript(command, False)
            )
            new_ids = list(all_object_ids() - before)
            value = {
                "objects": describe(new_ids),
                "source_selection": selected_labels(),
                "succeeded": succeeded,
            }
            delete_objects(new_ids)
            return value

        try:
            for label in ("A", "B"):
                layer = Rhino.DocObjects.Layer()
                layer.Name = "ViboPointCloud%s%s" % (label, suffix)
                layer_index = document.Layers.Add(layer)
                if layer_index < 0:
                    raise ValueError("could not add point-cloud cycle layer")
                layer_indices.append(layer_index)

            line_attributes = Rhino.DocObjects.ObjectAttributes()
            line_attributes.LayerIndex = layer_indices[0]
            line_attributes.Name = "LineSource"
            line_id = document.Objects.AddLine(
                Rhino.Geometry.Point3d(0.0, 0.0, 0.0),
                Rhino.Geometry.Point3d(2.0, 0.0, 0.0),
                line_attributes,
            )
            mesh_attributes = Rhino.DocObjects.ObjectAttributes()
            mesh_attributes.LayerIndex = layer_indices[1]
            mesh_attributes.Name = "MeshSource"
            mesh_id = document.Objects.AddMesh(
                _triangle_mesh(
                    [[10.0, 0.0, 0.0], [12.0, 0.0, 0.0], [10.0, 2.0, 0.0]],
                    [[0, 1, 2]],
                ),
                mesh_attributes,
            )
            cloud_attributes = Rhino.DocObjects.ObjectAttributes()
            cloud_attributes.LayerIndex = layer_indices[0]
            cloud_attributes.Name = "CloudSource"
            cloud_id = document.Objects.AddPointCloud(
                System.Array[Rhino.Geometry.Point3d](
                    [
                        Rhino.Geometry.Point3d(20.0, 0.0, 0.0),
                        Rhino.Geometry.Point3d(21.0, 1.0, 0.0),
                        Rhino.Geometry.Point3d(22.0, 0.0, 0.0),
                    ]
                ),
                cloud_attributes,
            )
            point_attributes = Rhino.DocObjects.ObjectAttributes()
            point_attributes.LayerIndex = layer_indices[1]
            point_attributes.Name = "PointSource"
            point_id = document.Objects.AddPoint(
                Rhino.Geometry.Point3d(30.0, 0.0, 0.0), point_attributes
            )
            source_ids.extend([line_id, mesh_id, cloud_id, point_id])
            if any(object_id == System.Guid.Empty for object_id in source_ids):
                raise ValueError("could not add point-cloud cycle source")

            value = {
                "cloud_to_cloud_input": run_extract(
                    [cloud_id], "_PointCloud", "_Input"
                ),
                "line_mesh_cloud_current": run_extract(
                    [line_id, mesh_id, cloud_id], "_PointCloud", "_Current"
                ),
                "mesh_line_cloud_input": run_extract(
                    [mesh_id, line_id], "_PointCloud", "_Input"
                ),
            }

            document.Objects.UnselectAll()
            value["sel_pt_succeeded"] = bool(
                Rhino.RhinoApp.RunScript("_-SelPt", False)
            )
            value["sel_pt"] = selected_labels()
            document.Objects.UnselectAll()
            value["sel_pt_cloud_succeeded"] = bool(
                Rhino.RhinoApp.RunScript("_-SelPtCloud", False)
            )
            value["sel_pt_cloud"] = selected_labels()

            document.Objects.UnselectAll()
            before_explode = all_object_ids()
            value["explode_succeeded"] = bool(
                Rhino.RhinoApp.RunScript(
                    "_-Explode _SelID %s _Enter" % cloud_id, False
                )
            )
            exploded_ids = list(all_object_ids() - before_explode)
            value["explode"] = describe(exploded_ids)
            value["explode_source_exists"] = document.Objects.FindId(cloud_id) is not None
            delete_objects(exploded_ids)

            equality_deltas = [
                1.0e-16,
                1.0e-15,
                1.0e-14,
                1.0e-13,
                1.0e-12,
                1.0e-11,
                1.0e-10,
                1.0e-9,
                1.0e-8,
                1.0e-7,
            ]
            equality_base = Rhino.Geometry.PointCloud(
                [
                    Rhino.Geometry.Point3d(1.0, 2.0, 3.0),
                    Rhino.Geometry.Point3d(4.0, 5.0, 6.0),
                ]
            )

            def point_cloud_geometry_equals(left, right_points):
                right = Rhino.Geometry.PointCloud(right_points)
                try:
                    return bool(
                        Rhino.Geometry.GeometryBase.GeometryEquals(left, right)
                    )
                finally:
                    right.Dispose()

            try:
                value["geometry_equals_delta"] = [
                    point_cloud_geometry_equals(
                        equality_base,
                        [
                            Rhino.Geometry.Point3d(1.0 + delta, 2.0, 3.0),
                            Rhino.Geometry.Point3d(4.0, 5.0, 6.0),
                        ],
                    )
                    for delta in equality_deltas
                ]
                value["geometry_equals_reversed"] = point_cloud_geometry_equals(
                    equality_base,
                    [
                        Rhino.Geometry.Point3d(4.0, 5.0, 6.0),
                        Rhino.Geometry.Point3d(1.0, 2.0, 3.0),
                    ],
                )
                relative_equals = []
                for scale in (1.0, 1.0e3, 1.0e6, 1.0e9):
                    relative_base = Rhino.Geometry.PointCloud(
                        [
                            Rhino.Geometry.Point3d(scale, 0.0, 0.0),
                            Rhino.Geometry.Point3d(0.0, scale, 0.0),
                        ]
                    )
                    try:
                        relative_equals.append(
                            point_cloud_geometry_equals(
                                relative_base,
                                [
                                    Rhino.Geometry.Point3d(
                                        scale * (1.0 + 1.0e-10), 0.0, 0.0
                                    ),
                                    Rhino.Geometry.Point3d(0.0, scale, 0.0),
                                ],
                            )
                        )
                    finally:
                        relative_base.Dispose()
                value["geometry_equals_relative_delta"] = relative_equals
            finally:
                equality_base.Dispose()

            query_cloud = Rhino.Geometry.PointCloud(
                [
                    Rhino.Geometry.Point3d(
                        float(index % 64), float(index // 64), 0.0
                    )
                    for index in iteration_range(4096)
                ]
            )
            query = Rhino.Geometry.Point3d(31.25, 27.75, 0.0)
            _unused, elapsed = _measure(
                iterations, lambda: query_cloud.ClosestPoint(query)
            )
            query_cloud.Dispose()
            return value, elapsed
        finally:
            document.Objects.UnselectAll()
            for object_id in source_ids:
                document.Objects.Delete(object_id, True)
            for layer_index in reversed(layer_indices):
                document.Layers.Delete(layer_index, True)
    if kind == "document_layer_assignment_cycle":
        document = Rhino.RhinoDoc.ActiveDoc
        suffix = str(System.Guid.NewGuid()).replace("-", "")
        default_layer_index = int(document.Layers.CurrentLayerIndex)
        layer_indices = []
        object_ids = []
        original_group_index = None

        def set_layer_mode(layer_index, visible, locked):
            layer = document.Layers[layer_index]
            layer.IsVisible = visible
            layer.IsLocked = locked
            if not document.Layers.Modify(layer, layer_index, True):
                raise ValueError("could not modify layer-assignment layer")

        try:
            for label, visible, locked in (
                ("Normal", True, False),
                ("Hidden", False, False),
                ("Locked", True, True),
            ):
                layer = Rhino.DocObjects.Layer()
                layer.Name = "ViboLayerAssignment%s%s" % (label, suffix)
                layer.IsVisible = visible
                layer.IsLocked = locked
                layer_index = document.Layers.Add(layer)
                if layer_index < 0:
                    raise ValueError("could not add layer-assignment layer")
                layer_indices.append(layer_index)

            for index in range(5):
                attributes = Rhino.DocObjects.ObjectAttributes()
                attributes.LayerIndex = default_layer_index
                attributes.Name = "Part%d" % index
                object_id = document.Objects.AddPoint(
                    Rhino.Geometry.Point3d(float(index), 0.0, 0.0), attributes
                )
                if object_id == System.Guid.Empty:
                    raise ValueError("could not add layer-assignment point")
                object_ids.append(object_id)
            original_group_index = document.Groups.Add(
                "ViboLayerAssignmentAssembly" + suffix, object_ids[:2]
            )
            if original_group_index < 0:
                raise ValueError("could not add layer-assignment group")

            layer_labels = {
                default_layer_index: "Default",
                layer_indices[0]: "Normal",
                layer_indices[1]: "Hidden",
                layer_indices[2]: "Locked",
            }

            def point_ids():
                settings = Rhino.DocObjects.ObjectEnumeratorSettings()
                settings.NormalObjects = True
                settings.LockedObjects = True
                settings.HiddenObjects = True
                settings.ObjectTypeFilter = Rhino.DocObjects.ObjectType.Point
                return set(
                    obj.Id for obj in document.Objects.GetObjectList(settings)
                )

            def selected_indices(ids):
                return [
                    index
                    for index, object_id in enumerate(ids)
                    if document.Objects.FindId(object_id).IsSelected(False) != 0
                ]

            def select_only(ids):
                document.Objects.UnselectAll()
                for object_id in ids:
                    if not document.Objects.Select(object_id):
                        raise ValueError(
                            "could not preselect layer-assignment object"
                        )

            def object_layer_indices(ids):
                return [
                    int(document.Objects.FindId(object_id).Attributes.LayerIndex)
                    for object_id in ids
                ]

            def object_layer_labels(ids):
                labels = []
                for layer_index in object_layer_indices(ids):
                    if layer_index not in layer_labels:
                        raise ValueError(
                            "layer-assignment object used an unexpected layer"
                        )
                    labels.append(layer_labels[layer_index])
                return labels

            def group_indices_for(ids):
                ids = set(ids)
                result = []
                for group_index in range(document.Groups.Count):
                    members = document.Groups.GroupMembers(group_index)
                    if members is None:
                        continue
                    if any(member.Id in ids for member in members):
                        result.append(group_index)
                return result

            def group_sizes_for(ids):
                ids = set(ids)
                sizes = []
                for group_index in group_indices_for(ids):
                    members = document.Groups.GroupMembers(group_index)
                    size = sum(1 for member in members if member.Id in ids)
                    if size:
                        sizes.append(size)
                return sorted(sizes)

            def modify_object_layers(ids, layer_index):
                changed = 0
                for object_id in ids:
                    rhino_object = document.Objects.FindId(object_id)
                    if rhino_object is None:
                        raise ValueError("layer-assignment object disappeared")
                    if int(rhino_object.Attributes.LayerIndex) == layer_index:
                        continue
                    attributes = rhino_object.Attributes.Duplicate()
                    attributes.LayerIndex = layer_index
                    if not document.Objects.ModifyAttributes(
                        rhino_object, attributes, True
                    ):
                        raise ValueError(
                            "could not modify layer-assignment object"
                        )
                    changed += 1
                return changed

            def run_layer_command(command_name, layer_index):
                layer_name = document.Layers[layer_index].Name
                if not Rhino.RhinoApp.RunScript(
                    "_-%s %s _Enter" % (command_name, layer_name), False
                ):
                    raise ValueError(
                        "%s failed in layer-assignment cycle" % command_name
                    )

            def new_point_ids(before):
                ids = list(point_ids() - before)
                ids.sort(
                    key=lambda object_id: document.Objects.FindId(
                        object_id
                    ).Geometry.Location.X
                )
                return ids

            def reset_from_destination(ids, layer_index):
                layer = document.Layers[layer_index]
                visible = bool(layer.IsVisible)
                locked = bool(layer.IsLocked)
                if not visible or locked:
                    set_layer_mode(layer_index, True, False)
                modify_object_layers(ids, default_layer_index)
                if not visible or locked:
                    set_layer_mode(layer_index, visible, locked)

            def delete_copies(ids, layer_index):
                for group_index in reversed(group_indices_for(ids)):
                    document.Groups.Delete(group_index)
                layer = document.Layers[layer_index]
                visible = bool(layer.IsVisible)
                locked = bool(layer.IsLocked)
                if not visible or locked:
                    set_layer_mode(layer_index, True, False)
                for object_id in ids:
                    document.Objects.Show(object_id, True)
                    document.Objects.Unlock(object_id, True)
                    if not document.Objects.Delete(object_id, True):
                        raise ValueError("could not delete layer-assignment copy")
                if not visible or locked:
                    set_layer_mode(layer_index, visible, locked)

            current_before = int(document.Layers.CurrentLayerIndex)
            select_only(object_ids[:2])
            before_change_layers = object_layer_indices(object_ids[:2])
            run_layer_command("ChangeLayer", layer_indices[0])
            after_change_layers = object_layer_indices(object_ids[:2])
            change_count = sum(
                1
                for before, after in zip(
                    before_change_layers, after_change_layers
                )
                if before != after
            )
            change_layers = object_layer_labels(object_ids[:2])
            change_selected = selected_indices(object_ids)
            change_group_sizes = group_sizes_for(object_ids[:2])
            current_after_change = (
                "Default"
                if int(document.Layers.CurrentLayerIndex)
                == default_layer_index
                else "Unexpected"
            )
            reset_from_destination(object_ids[:2], layer_indices[0])

            before_copy = point_ids()
            select_only(object_ids[:2])
            run_layer_command("CopyToLayer", layer_indices[0])
            copy_ids = new_point_ids(before_copy)
            copy_count = len(copy_ids)
            copy_layers = object_layer_labels(copy_ids)
            copy_names = [
                document.Objects.FindId(object_id).Attributes.Name
                for object_id in copy_ids
            ]
            copy_group_sizes = group_sizes_for(copy_ids)
            copy_selected = selected_indices(copy_ids)
            original_selected_after_copy = selected_indices(object_ids)
            delete_copies(copy_ids, layer_indices[0])

            modify_object_layers([object_ids[0]], layer_indices[0])
            before_mixed_copy = point_ids()
            select_only(object_ids[:2])
            run_layer_command("CopyToLayer", layer_indices[0])
            mixed_copy_ids = new_point_ids(before_mixed_copy)
            mixed_copy_count = len(mixed_copy_ids)
            mixed_copy_layers = object_layer_labels(mixed_copy_ids)
            mixed_copy_group_sizes = group_sizes_for(mixed_copy_ids)
            delete_copies(mixed_copy_ids, layer_indices[0])
            reset_from_destination([object_ids[0]], layer_indices[0])

            modify_object_layers(object_ids[:2], layer_indices[0])
            before_same_layer_copy = point_ids()
            select_only(object_ids[:2])
            run_layer_command("CopyToLayer", layer_indices[0])
            same_layer_copy_ids = new_point_ids(before_same_layer_copy)
            same_layer_copy_count = len(same_layer_copy_ids)
            delete_copies(same_layer_copy_ids, layer_indices[0])
            reset_from_destination(object_ids[:2], layer_indices[0])

            select_only([object_ids[2]])
            before_hidden_change = object_layer_indices([object_ids[2]])
            run_layer_command("ChangeLayer", layer_indices[1])
            hidden_change_count = int(
                before_hidden_change
                != object_layer_indices([object_ids[2]])
            )
            hidden_change_selected = selected_indices(object_ids)
            reset_from_destination([object_ids[2]], layer_indices[1])

            select_only([object_ids[3]])
            before_locked_change = object_layer_indices([object_ids[3]])
            run_layer_command("ChangeLayer", layer_indices[2])
            locked_change_count = int(
                before_locked_change
                != object_layer_indices([object_ids[3]])
            )
            locked_change_selected = selected_indices(object_ids)
            reset_from_destination([object_ids[3]], layer_indices[2])

            before_hidden_copy = point_ids()
            select_only([object_ids[2]])
            run_layer_command("CopyToLayer", layer_indices[1])
            hidden_copy_ids = new_point_ids(before_hidden_copy)
            hidden_copy_count = len(hidden_copy_ids)
            hidden_copy_layers = object_layer_labels(hidden_copy_ids)
            hidden_copy_selected = selected_indices(hidden_copy_ids)
            delete_copies(hidden_copy_ids, layer_indices[1])

            before_locked_copy = point_ids()
            select_only([object_ids[3]])
            run_layer_command("CopyToLayer", layer_indices[2])
            locked_copy_ids = new_point_ids(before_locked_copy)
            locked_copy_count = len(locked_copy_ids)
            locked_copy_layers = object_layer_labels(locked_copy_ids)
            locked_copy_selected = selected_indices(locked_copy_ids)
            original_selected_after_destination_copies = selected_indices(
                object_ids
            )
            delete_copies(locked_copy_ids, layer_indices[2])

            value = {
                "change_count": change_count,
                "change_group_sizes": change_group_sizes,
                "change_layers": change_layers,
                "change_selected": change_selected,
                "copy_count": copy_count,
                "copy_group_sizes": copy_group_sizes,
                "copy_layers": copy_layers,
                "copy_names": copy_names,
                "copy_selected": copy_selected,
                "current_after_change": current_after_change,
                "current_unchanged": (
                    int(document.Layers.CurrentLayerIndex) == current_before
                ),
                "hidden_change_count": hidden_change_count,
                "hidden_change_selected": hidden_change_selected,
                "hidden_copy_count": hidden_copy_count,
                "hidden_copy_layers": hidden_copy_layers,
                "hidden_copy_selected": hidden_copy_selected,
                "locked_change_count": locked_change_count,
                "locked_change_selected": locked_change_selected,
                "locked_copy_count": locked_copy_count,
                "locked_copy_layers": locked_copy_layers,
                "locked_copy_selected": locked_copy_selected,
                "mixed_copy_count": mixed_copy_count,
                "mixed_copy_group_sizes": mixed_copy_group_sizes,
                "mixed_copy_layers": mixed_copy_layers,
                "original_selected_after_copy": original_selected_after_copy,
                "original_selected_after_destination_copies": (
                    original_selected_after_destination_copies
                ),
                "same_layer_copy_count": same_layer_copy_count,
            }

            modify_object_layers(object_ids[:2], layer_indices[0])
            modify_object_layers(object_ids[:2], default_layer_index)
            started = default_timer()
            for _unused in iteration_range(iterations):
                modify_object_layers(object_ids[:2], layer_indices[0])
                modify_object_layers(object_ids[:2], default_layer_index)
            elapsed_ns = int(
                round((default_timer() - started) * 1000000000.0)
            )
            return value, max(0, elapsed_ns)
        finally:
            document.Objects.UnselectAll()
            for layer_index in layer_indices:
                layer = document.Layers[layer_index]
                if layer is not None and (
                    not layer.IsVisible or layer.IsLocked
                ):
                    set_layer_mode(layer_index, True, False)
            if original_group_index is not None and original_group_index >= 0:
                document.Groups.Delete(original_group_index)
            for object_id in object_ids:
                document.Objects.Show(object_id, True)
                document.Objects.Unlock(object_id, True)
                document.Objects.Delete(object_id, True)
            for layer_index in reversed(layer_indices):
                document.Layers.Delete(layer_index, True)
    if kind == "document_object_state_cycle":
        object_count_value = operation.get("object_count")
        if (
            isinstance(object_count_value, bool)
            or int(object_count_value) != object_count_value
        ):
            raise ValueError("object_count must be an integer")
        object_count = int(object_count_value)
        if object_count < 1 or object_count > MAX_STATE_CYCLE_OBJECTS:
            raise ValueError(
                "object_count must be from 1 through %d"
                % MAX_STATE_CYCLE_OBJECTS
            )
        hide_indices = _state_cycle_indices(
            operation, "hide_indices", object_count
        )
        lock_indices = _state_cycle_indices(
            operation, "lock_indices", object_count
        )
        document = Rhino.RhinoDoc.ActiveDoc
        object_ids = []
        try:
            for index in range(object_count):
                object_id = document.Objects.AddPoint(
                    Rhino.Geometry.Point3d(float(index), 0.0, 0.0)
                )
                if object_id == System.Guid.Empty:
                    raise ValueError("could not add document state-cycle point")
                object_ids.append(object_id)

            hide_ids = [object_ids[index] for index in hide_indices]
            lock_ids = [object_ids[index] for index in lock_indices]

            def object_modes():
                modes = []
                for object_id in object_ids:
                    rhino_object = document.Objects.FindId(object_id)
                    if rhino_object is None:
                        raise ValueError("document state-cycle object disappeared")
                    if rhino_object.IsHidden:
                        modes.append("hidden")
                    elif rhino_object.IsLocked:
                        modes.append("locked")
                    else:
                        modes.append("normal")
                return modes

            def state_cycle():
                document.Objects.UnselectAll()
                for object_id in hide_ids:
                    if not document.Objects.Select(object_id):
                        raise ValueError("could not select object to hide")
                hide_count = sum(
                    1
                    for object_id in hide_ids
                    if document.Objects.Hide(object_id, True)
                )
                modes_after_hide = object_modes()
                selected_after_hide = int(
                    document.Objects.GetSelectedObjectCount(False)
                )

                show_count = sum(
                    1
                    for object_id in hide_ids
                    if document.Objects.Show(object_id, True)
                )
                modes_after_show = object_modes()
                for object_id in lock_ids:
                    if not document.Objects.Select(object_id):
                        raise ValueError("could not select object to lock")
                lock_count = sum(
                    1
                    for object_id in lock_ids
                    if document.Objects.Lock(object_id, True)
                )
                modes_after_lock = object_modes()
                selected_after_lock = int(
                    document.Objects.GetSelectedObjectCount(False)
                )

                unlock_count = sum(
                    1
                    for object_id in lock_ids
                    if document.Objects.Unlock(object_id, True)
                )
                modes_after_unlock = object_modes()
                return {
                    "hide_count": hide_count,
                    "lock_count": lock_count,
                    "modes_after_hide": modes_after_hide,
                    "modes_after_lock": modes_after_lock,
                    "modes_after_show": modes_after_show,
                    "modes_after_unlock": modes_after_unlock,
                    "selected_after_hide": selected_after_hide,
                    "selected_after_lock": selected_after_lock,
                    "show_count": show_count,
                    "unlock_count": unlock_count,
                }

            return _measure(iterations, state_cycle)
        finally:
            document.Objects.UnselectAll()
            for object_id in object_ids:
                document.Objects.Show(object_id, True)
                document.Objects.Unlock(object_id, True)
                document.Objects.Delete(object_id, True)

    if kind == "document_object_swap_cycle":
        document = Rhino.RhinoDoc.ActiveDoc
        object_ids = []
        try:
            for index in range(3):
                object_id = document.Objects.AddPoint(
                    Rhino.Geometry.Point3d(float(index), 0.0, 0.0)
                )
                if object_id == System.Guid.Empty:
                    raise ValueError("could not add document swap-cycle point")
                object_ids.append(object_id)
            if not document.Objects.Hide(object_ids[1], True):
                raise ValueError("could not seed hidden object")
            if not document.Objects.Lock(object_ids[2], True):
                raise ValueError("could not seed locked object")

            layer_suffix = str(System.Guid.NewGuid())
            hidden_layer = Rhino.DocObjects.Layer()
            hidden_layer.Name = "Viboceros Swap Cycle Hidden " + layer_suffix
            hidden_layer.IsVisible = False
            hidden_layer_index = document.Layers.Add(hidden_layer)
            locked_layer = Rhino.DocObjects.Layer()
            locked_layer.Name = "Viboceros Swap Cycle Locked " + layer_suffix
            locked_layer.IsLocked = True
            locked_layer_index = document.Layers.Add(locked_layer)
            if hidden_layer_index < 0 or locked_layer_index < 0:
                raise ValueError("could not add document swap-cycle layers")
            for layer_index in (hidden_layer_index, locked_layer_index):
                for mode_index in range(3):
                    attributes = Rhino.DocObjects.ObjectAttributes()
                    attributes.LayerIndex = layer_index
                    object_id = document.Objects.AddPoint(
                        Rhino.Geometry.Point3d(float(len(object_ids)), 0.0, 0.0),
                        attributes,
                    )
                    if object_id == System.Guid.Empty:
                        raise ValueError("could not add layered swap-cycle point")
                    object_ids.append(object_id)
                    if mode_index == 1 and not document.Objects.Hide(
                        object_id, True
                    ):
                        raise ValueError("could not seed layered hidden object")
                    if mode_index == 2 and not document.Objects.Lock(
                        object_id, True
                    ):
                        raise ValueError("could not seed layered locked object")

            def swap_modes():
                result = []
                for object_id in object_ids:
                    rhino_object = document.Objects.FindId(object_id)
                    if rhino_object.IsHidden:
                        result.append("hidden")
                    elif rhino_object.IsLocked:
                        result.append("locked")
                    else:
                        result.append("normal")
                return result

            def layer_allows_swap(rhino_object):
                layer = document.Layers[rhino_object.Attributes.LayerIndex]
                return bool(layer.IsVisible and not layer.IsLocked)

            def hide_swap():
                changed = 0
                for object_id in object_ids:
                    rhino_object = document.Objects.FindId(object_id)
                    if not layer_allows_swap(rhino_object):
                        continue
                    if rhino_object.IsHidden:
                        changed += int(document.Objects.Show(object_id, True))
                    elif not rhino_object.IsLocked:
                        changed += int(document.Objects.Hide(object_id, True))
                return changed

            def lock_swap():
                changed = 0
                for object_id in object_ids:
                    rhino_object = document.Objects.FindId(object_id)
                    if not layer_allows_swap(rhino_object) or rhino_object.IsHidden:
                        continue
                    if rhino_object.IsLocked:
                        changed += int(document.Objects.Unlock(object_id, True))
                    else:
                        changed += int(document.Objects.Lock(object_id, True))
                return changed

            labels = [
                "default-normal",
                "default-hidden",
                "default-locked",
                "hidden-layer-normal",
                "hidden-layer-hidden",
                "hidden-layer-locked",
                "locked-layer-normal",
                "locked-layer-hidden",
                "locked-layer-locked",
            ]

            def swap_cycle():
                document.Objects.UnselectAll()
                if not document.Objects.Select(object_ids[0]):
                    raise ValueError("could not select object before HideSwap")
                hide_count_once = hide_swap()
                hide_once = swap_modes()
                selected_after_hide = int(
                    document.Objects.GetSelectedObjectCount(False)
                )
                hide_count_twice = hide_swap()
                hide_twice = swap_modes()

                if not document.Objects.Select(object_ids[0]):
                    raise ValueError("could not select object before LockSwap")
                lock_count_once = lock_swap()
                lock_once = swap_modes()
                selected_after_lock = int(
                    document.Objects.GetSelectedObjectCount(False)
                )
                lock_count_twice = lock_swap()
                return {
                    "hide_count_once": hide_count_once,
                    "hide_count_twice": hide_count_twice,
                    "hide_once": hide_once,
                    "hide_twice": hide_twice,
                    "labels": labels,
                    "lock_count_once": lock_count_once,
                    "lock_count_twice": lock_count_twice,
                    "lock_once": lock_once,
                    "lock_twice": swap_modes(),
                    "selected_after_hide": selected_after_hide,
                    "selected_after_lock": selected_after_lock,
                }

            return _measure(iterations, swap_cycle)
        finally:
            document.Objects.UnselectAll()
            for object_id in object_ids:
                document.Objects.Show(object_id, True)
                document.Objects.Unlock(object_id, True)
                document.Objects.Delete(object_id, True)

    if kind == "document_object_isolation_cycle":
        document = Rhino.RhinoDoc.ActiveDoc
        object_ids = []
        isolated_hidden = []
        isolated_locked = []
        try:
            for index in range(4):
                object_id = document.Objects.AddPoint(
                    Rhino.Geometry.Point3d(float(index), 0.0, 0.0)
                )
                if object_id == System.Guid.Empty:
                    raise ValueError("could not add document isolation-cycle point")
                object_ids.append(object_id)
            if not document.Objects.Hide(object_ids[2], True):
                raise ValueError("could not seed isolation-cycle hidden object")
            if not document.Objects.Lock(object_ids[3], True):
                raise ValueError("could not seed isolation-cycle locked object")

            layer_suffix = str(System.Guid.NewGuid())
            hidden_layer = Rhino.DocObjects.Layer()
            hidden_layer.Name = "Viboceros Isolation Cycle Hidden " + layer_suffix
            hidden_layer.IsVisible = False
            hidden_layer_index = document.Layers.Add(hidden_layer)
            locked_layer = Rhino.DocObjects.Layer()
            locked_layer.Name = "Viboceros Isolation Cycle Locked " + layer_suffix
            locked_layer.IsLocked = True
            locked_layer_index = document.Layers.Add(locked_layer)
            if hidden_layer_index < 0 or locked_layer_index < 0:
                raise ValueError("could not add document isolation-cycle layers")
            for layer_index in (hidden_layer_index, locked_layer_index):
                for mode_index in range(3):
                    attributes = Rhino.DocObjects.ObjectAttributes()
                    attributes.LayerIndex = layer_index
                    object_id = document.Objects.AddPoint(
                        Rhino.Geometry.Point3d(float(len(object_ids)), 0.0, 0.0),
                        attributes,
                    )
                    if object_id == System.Guid.Empty:
                        raise ValueError(
                            "could not add layered isolation-cycle point"
                        )
                    object_ids.append(object_id)
                    if mode_index == 1 and not document.Objects.Hide(
                        object_id, True
                    ):
                        raise ValueError("could not seed layered hidden object")
                    if mode_index == 2 and not document.Objects.Lock(
                        object_id, True
                    ):
                        raise ValueError("could not seed layered locked object")

            def isolation_modes():
                result = []
                for object_id in object_ids:
                    rhino_object = document.Objects.FindId(object_id)
                    if rhino_object.IsHidden:
                        result.append("hidden")
                    elif rhino_object.IsLocked:
                        result.append("locked")
                    else:
                        result.append("normal")
                return result

            def layer_allows_isolation(rhino_object):
                layer = document.Layers[rhino_object.Attributes.LayerIndex]
                return bool(layer.IsVisible and not layer.IsLocked)

            def isolate():
                changed = 0
                for object_id in object_ids:
                    rhino_object = document.Objects.FindId(object_id)
                    if (
                        rhino_object.IsSelected(False) != 0
                        or rhino_object.IsHidden
                        or rhino_object.IsLocked
                        or not layer_allows_isolation(rhino_object)
                    ):
                        continue
                    if document.Objects.Hide(object_id, True):
                        isolated_hidden.append(object_id)
                        changed += 1
                return changed

            def unisolate():
                changed = sum(
                    1
                    for object_id in isolated_hidden
                    if document.Objects.Show(object_id, True)
                )
                del isolated_hidden[:]
                return changed

            def isolate_lock():
                changed = 0
                for object_id in object_ids:
                    rhino_object = document.Objects.FindId(object_id)
                    if (
                        rhino_object.IsSelected(False) != 0
                        or rhino_object.IsHidden
                        or rhino_object.IsLocked
                        or not layer_allows_isolation(rhino_object)
                    ):
                        continue
                    if document.Objects.Lock(object_id, True):
                        isolated_locked.append(object_id)
                        changed += 1
                return changed

            def unisolate_lock():
                changed = sum(
                    1
                    for object_id in isolated_locked
                    if document.Objects.Unlock(object_id, True)
                )
                del isolated_locked[:]
                return changed

            labels = [
                "default-selected",
                "default-normal",
                "default-hidden",
                "default-locked",
                "hidden-layer-normal",
                "hidden-layer-hidden",
                "hidden-layer-locked",
                "locked-layer-normal",
                "locked-layer-hidden",
                "locked-layer-locked",
            ]

            def isolation_cycle():
                document.Objects.UnselectAll()
                if not document.Objects.Select(object_ids[0]):
                    raise ValueError("could not select isolation survivor")
                isolate_count = isolate()
                isolate_repeat_count = isolate()
                after_isolate = isolation_modes()
                selected_after_isolate = int(
                    document.Objects.GetSelectedObjectCount(False)
                )
                unisolate_count = unisolate()
                unisolate_repeat_count = unisolate()
                after_unisolate = isolation_modes()
                selected_after_unisolate = int(
                    document.Objects.GetSelectedObjectCount(False)
                )

                if (
                    document.Objects.FindId(object_ids[0]).IsSelected(False) == 0
                    and not document.Objects.Select(object_ids[0])
                ):
                    raise ValueError("could not select isolation-lock survivor")
                isolate_lock_count = isolate_lock()
                isolate_lock_repeat_count = isolate_lock()
                after_isolate_lock = isolation_modes()
                selected_after_isolate_lock = int(
                    document.Objects.GetSelectedObjectCount(False)
                )
                unisolate_lock_count = unisolate_lock()
                unisolate_lock_repeat_count = unisolate_lock()
                selected_after_unisolate_lock = int(
                    document.Objects.GetSelectedObjectCount(False)
                )
                return {
                    "after_isolate": after_isolate,
                    "after_isolate_lock": after_isolate_lock,
                    "after_unisolate": after_unisolate,
                    "after_unisolate_lock": isolation_modes(),
                    "isolate_count": isolate_count,
                    "isolate_lock_count": isolate_lock_count,
                    "isolate_lock_repeat_count": isolate_lock_repeat_count,
                    "isolate_repeat_count": isolate_repeat_count,
                    "labels": labels,
                    "selected_after_isolate": selected_after_isolate,
                    "selected_after_isolate_lock": selected_after_isolate_lock,
                    "selected_after_unisolate": selected_after_unisolate,
                    "selected_after_unisolate_lock": selected_after_unisolate_lock,
                    "unisolate_count": unisolate_count,
                    "unisolate_lock_count": unisolate_lock_count,
                    "unisolate_lock_repeat_count": unisolate_lock_repeat_count,
                    "unisolate_repeat_count": unisolate_repeat_count,
                }

            return _measure(iterations, isolation_cycle)
        finally:
            document.Objects.UnselectAll()
            for object_id in object_ids:
                document.Objects.Show(object_id, True)
                document.Objects.Unlock(object_id, True)
                document.Objects.Delete(object_id, True)

    if kind == "document_action_selection_cycle":
        document = Rhino.RhinoDoc.ActiveDoc
        object_ids = []
        batch_ids = []
        state = {"previous": []}
        try:
            for index in range(4):
                object_id = document.Objects.AddPoint(
                    Rhino.Geometry.Point3d(float(index), 0.0, 0.0)
                )
                if object_id == System.Guid.Empty:
                    raise ValueError("could not add action-selection point")
                object_ids.append(object_id)
            for index in range(2):
                object_id = document.Objects.AddPoint(
                    Rhino.Geometry.Point3d(float(index), 1.0, 0.0)
                )
                if object_id == System.Guid.Empty:
                    raise ValueError("could not add batch action-selection point")
                batch_ids.append(object_id)
            all_ids = object_ids + batch_ids
            last_changed = [object_ids[3]]
            batch_last_changed = list(batch_ids)

            def current_selection():
                return [
                    object_id
                    for object_id in all_ids
                    if document.Objects.FindId(object_id).IsSelected(False) != 0
                ]

            def assign_selection(ids):
                ids = set(ids)
                document.Objects.UnselectAll()
                for object_id in all_ids:
                    if object_id in ids and not document.Objects.Select(object_id):
                        raise ValueError("could not select action-selection object")

            def apply_selection(ids, deselect_others):
                current = current_selection()
                target = set(ids)
                if deselect_others:
                    next_selection = [
                        object_id for object_id in all_ids if object_id in target
                    ]
                else:
                    combined = set(current)
                    combined.update(target)
                    next_selection = [
                        object_id for object_id in all_ids if object_id in combined
                    ]
                if not set(current).issubset(set(next_selection)):
                    state["previous"] = current
                assign_selection(next_selection)
                return len(next_selection)

            def select_previous(deselect_others):
                current = current_selection()
                previous = set(state["previous"])
                if deselect_others:
                    next_selection = [
                        object_id for object_id in all_ids if object_id in previous
                    ]
                else:
                    combined = set(current)
                    combined.update(previous)
                    next_selection = [
                        object_id for object_id in all_ids if object_id in combined
                    ]
                assign_selection(next_selection)
                state["previous"] = current
                return len(next_selection)

            def selected_indices(ids):
                return [
                    index
                    for index, object_id in enumerate(ids)
                    if document.Objects.FindId(object_id).IsSelected(False) != 0
                ]

            def establish_previous():
                apply_selection([object_ids[0], object_ids[1]], True)
                apply_selection([], True)
                apply_selection([object_ids[2]], False)

            def action_selection_cycle():
                apply_selection([], True)
                apply_selection([object_ids[0]], True)
                last_default_count = apply_selection(last_changed, True)
                last_default = selected_indices(object_ids)
                previous_once_count = select_previous(True)
                previous_once = selected_indices(object_ids)
                previous_twice_count = select_previous(True)
                previous_twice = selected_indices(object_ids)

                apply_selection([object_ids[0]], True)
                last_add_count = apply_selection(last_changed, False)
                last_add = selected_indices(object_ids)

                establish_previous()
                previous_default_count = select_previous(True)
                previous_default = selected_indices(object_ids)
                previous_default_twice_count = select_previous(True)
                previous_default_twice = selected_indices(object_ids)

                establish_previous()
                previous_add_count = select_previous(False)
                previous_add = selected_indices(object_ids)

                apply_selection([], True)
                batch_last_count = apply_selection(batch_last_changed, True)
                return {
                    "batch_last": selected_indices(batch_ids),
                    "batch_last_count": batch_last_count,
                    "last_add": last_add,
                    "last_add_count": last_add_count,
                    "last_default": last_default,
                    "last_default_count": last_default_count,
                    "previous_add": previous_add,
                    "previous_add_count": previous_add_count,
                    "previous_default": previous_default,
                    "previous_default_count": previous_default_count,
                    "previous_default_twice": previous_default_twice,
                    "previous_default_twice_count": previous_default_twice_count,
                    "previous_once": previous_once,
                    "previous_once_count": previous_once_count,
                    "previous_twice": previous_twice,
                    "previous_twice_count": previous_twice_count,
                }

            return _measure(iterations, action_selection_cycle)
        finally:
            document.Objects.UnselectAll()
            for object_id in object_ids + batch_ids:
                document.Objects.Delete(object_id, True)

    if kind == "document_attribute_selection_cycle":
        document = Rhino.RhinoDoc.ActiveDoc
        object_ids = []
        group_indices = []
        suffix = " " + str(System.Guid.NewGuid())
        hidden_layer = Rhino.DocObjects.Layer()
        hidden_layer.Name = "Hidden Parts" + suffix
        hidden_layer.Color = System.Drawing.Color.FromArgb(10, 20, 30)
        hidden_layer_index = document.Layers.Add(hidden_layer)
        locked_layer = Rhino.DocObjects.Layer()
        locked_layer.Name = "Locked Parts" + suffix
        locked_layer.Color = System.Drawing.Color.FromArgb(40, 50, 60)
        locked_layer_index = document.Layers.Add(locked_layer)
        if hidden_layer_index < 0 or locked_layer_index < 0:
            raise ValueError("could not add attribute-selection layers")
        default_layer_index = document.Layers.CurrentLayerIndex
        probe_layer_indices = [
            default_layer_index,
            hidden_layer_index,
            locked_layer_index,
        ]

        def set_layer_mode(layer_index, visible, locked):
            layer = document.Layers[layer_index]
            if layer.IsVisible == visible and layer.IsLocked == locked:
                return
            settings = Rhino.DocObjects.Layer()
            settings.CopyAttributesFrom(layer)
            settings.Name = layer.Name
            settings.ParentLayerId = layer.ParentLayerId
            settings.IsVisible = visible
            settings.IsLocked = locked
            if not document.Layers.Modify(settings, layer.Id, True):
                raise ValueError(
                    "could not change attribute-selection layer %d to visible=%r locked=%r"
                    % (layer_index, visible, locked)
                )

        try:
            specifications = [
                (default_layer_index, None),
                (default_layer_index, "BoltA"),
                (default_layer_index, "bolta"),
                (default_layer_index, "BoltLong"),
                (default_layer_index, "Peer"),
                (default_layer_index, "BoltA"),
                (default_layer_index, "BoltA"),
                (hidden_layer_index, "BoltA"),
                (hidden_layer_index, "BoltA"),
                (locked_layer_index, "BoltA"),
                (locked_layer_index, "BoltA"),
            ]
            for index, (layer_index, name) in enumerate(specifications):
                attributes = Rhino.DocObjects.ObjectAttributes()
                attributes.LayerIndex = layer_index
                attributes.Name = name
                if index in (0, 1, 5, 6):
                    attributes.ObjectColor = System.Drawing.Color.FromArgb(
                        10, 20, 30
                    )
                    attributes.ColorSource = (
                        Rhino.DocObjects.ObjectColorSource.ColorFromObject
                    )
                object_id = document.Objects.AddPoint(
                    Rhino.Geometry.Point3d(float(index), 0.0, 0.0), attributes
                )
                if object_id == System.Guid.Empty:
                    raise ValueError("could not add attribute-selection point")
                object_ids.append(object_id)
            for index in (5, 8):
                if not document.Objects.Hide(object_ids[index], True):
                    raise ValueError("could not hide attribute-selection point")
            for index in (6, 10):
                if not document.Objects.Lock(object_ids[index], True):
                    raise ValueError("could not lock attribute-selection point")

            for name, members in (
                ("Team" + suffix, [object_ids[1], object_ids[4], object_ids[6]]),
                ("team" + suffix, [object_ids[2]]),
                ("Overlap" + suffix, [object_ids[1], object_ids[3]]),
            ):
                group_index = document.Groups.Add(
                    name, System.Array[System.Guid](members)
                )
                if group_index < 0:
                    raise ValueError("could not add attribute-selection group")
                group_indices.append(group_index)

            def object_is_selectable(object_id):
                rhino_object = document.Objects.FindId(object_id)
                if (
                    rhino_object is None
                    or rhino_object.IsHidden
                    or rhino_object.IsLocked
                ):
                    return False
                layer = document.Layers[rhino_object.Attributes.LayerIndex]
                return bool(layer.IsVisible and not layer.IsLocked)

            def selected_indices():
                return [
                    index
                    for index, object_id in enumerate(object_ids)
                    if document.Objects.FindId(object_id).IsSelected(False) > 0
                ]

            def select_ids(ids):
                for object_id in ids:
                    rhino_object = document.Objects.FindId(object_id)
                    if (
                        object_is_selectable(object_id)
                        and rhino_object.IsSelected(False) <= 0
                        and not document.Objects.Select(object_id)
                    ):
                        raise ValueError("could not select attribute-selection point")
                return int(document.Objects.GetSelectedObjectCount(False))

            def select_name(pattern):
                matches = []
                for object_id in object_ids:
                    rhino_object = document.Objects.FindId(object_id)
                    name = rhino_object.Attributes.Name or ""
                    if _wildcard_matches(pattern, name):
                        matches.append(object_id)
                return select_ids(matches)

            def select_group(name):
                matches = []
                for group_index in group_indices:
                    if document.Groups.GroupName(group_index) != name:
                        continue
                    members = document.Groups.GroupMembers(group_index)
                    if members is not None:
                        matches.extend(member.Id for member in members)
                return select_ids(matches)

            def select_layers(pattern):
                matching_layers = []
                for layer_index in probe_layer_indices:
                    layer = document.Layers[layer_index]
                    if _wildcard_matches(pattern, layer.Name):
                        matching_layers.append(layer_index)
                        set_layer_mode(layer_index, True, False)
                return select_ids(
                    object_id
                    for object_id in object_ids
                    if document.Objects.FindId(object_id).Attributes.LayerIndex
                    in matching_layers
                )

            def select_color(red, green, blue):
                matches = []
                for object_id in object_ids:
                    rhino_object = document.Objects.FindId(object_id)
                    attributes = rhino_object.Attributes
                    if int(attributes.GroupCount) > 0:
                        continue
                    color = attributes.DrawColor(document)
                    if (int(color.R), int(color.G), int(color.B)) == (
                        red,
                        green,
                        blue,
                    ):
                        matches.append(object_id)
                return select_ids(matches)

            def seed_selection():
                document.Objects.UnselectAll()
                return select_ids([object_ids[0]])

            def attribute_selection_cycle():
                set_layer_mode(hidden_layer_index, False, False)
                set_layer_mode(locked_layer_index, True, True)

                seed_selection()
                name_count = select_name("BOLT?")
                name = selected_indices()

                seed_selection()
                group_upper_count = select_group("Team" + suffix)
                group_upper = selected_indices()
                group_lower_count = select_group("team" + suffix)
                group_lower = selected_indices()
                group_wrong_case_count = select_group("TEAM" + suffix)
                group_wrong_case = selected_indices()

                seed_selection()
                hidden_layer_count = select_layers("hidden parts*")
                hidden_layer_selection = selected_indices()
                hidden_layer_visible = bool(
                    document.Layers[hidden_layer_index].IsVisible
                )
                locked_layer_count = select_layers("LOCKED*")
                locked_layer_selection = selected_indices()
                locked_layer_locked = bool(
                    document.Layers[locked_layer_index].IsLocked
                )
                all_layers_count = select_layers("*")
                all_layers = selected_indices()

                document.Objects.UnselectAll()
                select_ids([object_ids[9]])
                color_count = select_color(10, 20, 30)
                color = selected_indices()
                return {
                    "all_layers": all_layers,
                    "all_layers_count": all_layers_count,
                    "color": color,
                    "color_count": color_count,
                    "group_lower": group_lower,
                    "group_lower_count": group_lower_count,
                    "group_upper": group_upper,
                    "group_upper_count": group_upper_count,
                    "group_wrong_case": group_wrong_case,
                    "group_wrong_case_count": group_wrong_case_count,
                    "hidden_layer": hidden_layer_selection,
                    "hidden_layer_count": hidden_layer_count,
                    "hidden_layer_visible": hidden_layer_visible,
                    "locked_layer": locked_layer_selection,
                    "locked_layer_count": locked_layer_count,
                    "locked_layer_locked": locked_layer_locked,
                    "name": name,
                    "name_count": name_count,
                }

            return _measure(iterations, attribute_selection_cycle)
        finally:
            document.Objects.UnselectAll()
            for group_index in reversed(group_indices):
                document.Groups.Delete(group_index)
            set_layer_mode(hidden_layer_index, True, False)
            set_layer_mode(locked_layer_index, True, False)
            for object_id in object_ids:
                document.Objects.Show(object_id, True)
                document.Objects.Unlock(object_id, True)
                document.Objects.Delete(object_id, True)

    if kind == "document_object_naming_cycle":
        document = Rhino.RhinoDoc.ActiveDoc
        object_ids = []
        try:
            for index in range(3):
                object_id = document.Objects.AddPoint(
                    Rhino.Geometry.Point3d(float(index), 0.0, 0.0)
                )
                if object_id == System.Guid.Empty:
                    raise ValueError("could not add document naming-cycle point")
                object_ids.append(object_id)

            def object_names():
                return [
                    document.Objects.FindId(object_id).Attributes.Name
                    for object_id in object_ids
                ]

            def assign_names(names):
                changed = 0
                for object_id, name in zip(object_ids, names):
                    rhino_object = document.Objects.FindId(object_id)
                    attributes = rhino_object.Attributes.Duplicate()
                    attributes.Name = name
                    changed += int(
                        document.Objects.ModifyAttributes(
                            rhino_object, attributes, True
                        )
                    )
                return changed

            def naming_cycle():
                shared_count = assign_names(["Sample", "Sample", "Sample"])
                shared = object_names()
                counter_count = assign_names(
                    ["Sample 0", "Sample 1", "Sample 2"]
                )
                counter = object_names()
                clear_count = assign_names([None, None, None])
                return {
                    "clear_count": clear_count,
                    "cleared": object_names(),
                    "counter": counter,
                    "counter_count": counter_count,
                    "shared": shared,
                    "shared_count": shared_count,
                }

            return _measure(iterations, naming_cycle)
        finally:
            document.Objects.UnselectAll()
            for object_id in object_ids:
                document.Objects.Delete(object_id, True)

    if kind == "document_duplicate_selection_cycle":
        geometries = []
        selectable = []

        def remember(geometry, is_selectable=True):
            if geometry is None or not geometry.IsValid:
                raise ValueError("could not create duplicate-selection geometry")
            geometries.append(geometry)
            selectable.append(bool(is_selectable))
            return len(geometries) - 1

        try:
            remember(Rhino.Geometry.Point(Rhino.Geometry.Point3d(30.0, 0.0, 0.0)))
            point_original = remember(
                Rhino.Geometry.Point(Rhino.Geometry.Point3d(0.0, 0.0, 0.0))
            )
            remember(Rhino.Geometry.Point(Rhino.Geometry.Point3d(0.0, 0.0, 0.0)))
            remember(Rhino.Geometry.Point(Rhino.Geometry.Point3d(0.0, 0.0, 0.0)))
            remember(
                Rhino.Geometry.Point(Rhino.Geometry.Point3d(0.0, 0.0, 0.0)),
                False,
            )
            remember(
                Rhino.Geometry.Point(Rhino.Geometry.Point3d(0.0, 0.0, 0.0)),
                False,
            )
            point_near = remember(
                Rhino.Geometry.Point(
                    Rhino.Geometry.Point3d(
                        0.5 * tolerance["absolute"], 0.0, 0.0
                    )
                )
            )
            remember(Rhino.Geometry.Point(Rhino.Geometry.Point3d(20.0, 0.0, 0.0)))

            line_start = Rhino.Geometry.Point3d(0.0, 10.0, 0.0)
            line_end = Rhino.Geometry.Point3d(5.0, 10.0, 0.0)
            line_original = remember(Rhino.Geometry.LineCurve(line_start, line_end))
            remember(Rhino.Geometry.LineCurve(line_start, line_end))
            line_reversed = remember(Rhino.Geometry.LineCurve(line_end, line_start))
            line_nurbs_geometry = Rhino.Geometry.NurbsCurve.Create(
                False,
                1,
                System.Array[Rhino.Geometry.Point3d]([line_start, line_end]),
            )
            line_nurbs = remember(line_nurbs_geometry)
            line_near = remember(
                Rhino.Geometry.LineCurve(
                    line_start,
                    Rhino.Geometry.Point3d(
                        5.0 + 0.5 * tolerance["absolute"], 10.0, 0.0
                    ),
                )
            )

            open_vertices = [
                Rhino.Geometry.Point3d(0.0, 20.0, 0.0),
                Rhino.Geometry.Point3d(2.0, 20.0, 0.0),
                Rhino.Geometry.Point3d(2.0, 22.0, 0.0),
            ]
            open_polyline = remember(
                Rhino.Geometry.PolylineCurve(
                    System.Array[Rhino.Geometry.Point3d](open_vertices)
                )
            )
            remember(
                Rhino.Geometry.PolylineCurve(
                    System.Array[Rhino.Geometry.Point3d](open_vertices)
                )
            )
            open_polyline_reversed = remember(
                Rhino.Geometry.PolylineCurve(
                    System.Array[Rhino.Geometry.Point3d](list(reversed(open_vertices)))
                )
            )

            closed_vertices = [
                Rhino.Geometry.Point3d(10.0, 20.0, 0.0),
                Rhino.Geometry.Point3d(12.0, 20.0, 0.0),
                Rhino.Geometry.Point3d(12.0, 22.0, 0.0),
                Rhino.Geometry.Point3d(10.0, 20.0, 0.0),
            ]
            closed_polyline = remember(
                Rhino.Geometry.PolylineCurve(
                    System.Array[Rhino.Geometry.Point3d](closed_vertices)
                )
            )
            shifted_closed_polyline = remember(
                Rhino.Geometry.PolylineCurve(
                    System.Array[Rhino.Geometry.Point3d](
                        [
                            closed_vertices[1],
                            closed_vertices[2],
                            closed_vertices[0],
                            closed_vertices[1],
                        ]
                    )
                )
            )

            circle_center = Rhino.Geometry.Point3d(0.0, 30.0, 0.0)
            circle_plane = Rhino.Geometry.Plane(
                circle_center,
                Rhino.Geometry.Vector3d.XAxis,
                Rhino.Geometry.Vector3d.YAxis,
            )
            opposite_circle_plane = Rhino.Geometry.Plane(
                circle_center,
                Rhino.Geometry.Vector3d.XAxis,
                -Rhino.Geometry.Vector3d.YAxis,
            )
            circle_original = remember(
                Rhino.Geometry.ArcCurve(Rhino.Geometry.Circle(circle_plane, 3.0))
            )
            remember(Rhino.Geometry.ArcCurve(Rhino.Geometry.Circle(circle_plane, 3.0)))
            circle_opposite = remember(
                Rhino.Geometry.ArcCurve(
                    Rhino.Geometry.Circle(opposite_circle_plane, 3.0)
                )
            )

            mesh_vertices = [
                [0.0, 40.0, 0.0],
                [2.0, 40.0, 0.0],
                [0.0, 42.0, 0.0],
            ]
            mesh_original = remember(
                _triangle_mesh(mesh_vertices, [[0, 1, 2]])
            )
            remember(_triangle_mesh(mesh_vertices, [[0, 1, 2]]))
            mesh_reversed = remember(
                _triangle_mesh(mesh_vertices, [[0, 2, 1]])
            )
            mesh_reindexed = remember(
                _triangle_mesh(
                    [mesh_vertices[1], mesh_vertices[2], mesh_vertices[0]],
                    [[2, 0, 1]],
                )
            )

            def geometry_equal(left, right):
                return bool(
                    Rhino.Geometry.GeometryBase.GeometryEquals(
                        geometries[left], geometries[right]
                    )
                )

            def duplicate_classes():
                classes = []
                for index in range(len(geometries)):
                    if not selectable[index]:
                        continue
                    matching = None
                    for candidate_class in classes:
                        if geometry_equal(index, candidate_class[0]):
                            matching = candidate_class
                            break
                    if matching is None:
                        classes.append([index])
                    else:
                        matching.append(index)
                return [candidate_class for candidate_class in classes if len(candidate_class) > 1]

            def duplicate_selection_cycle():
                classes = duplicate_classes()
                selected_all = set([0])
                selected_without_originals = set([0])
                for candidate_class in classes:
                    selected_all.update(candidate_class)
                    selected_without_originals.update(candidate_class[1:])
                return {
                    "all": sorted(selected_all),
                    "all_count": len(selected_all),
                    "circle_opposite_equal": geometry_equal(
                        circle_original, circle_opposite
                    ),
                    "closed_shifted_equal": geometry_equal(
                        closed_polyline, shifted_closed_polyline
                    ),
                    "line_nurbs_equal": geometry_equal(
                        line_original, line_nurbs
                    ),
                    "line_near_equal": geometry_equal(line_original, line_near),
                    "line_reversed_equal": geometry_equal(
                        line_original, line_reversed
                    ),
                    "mesh_reindexed_equal": geometry_equal(
                        mesh_original, mesh_reindexed
                    ),
                    "mesh_reversed_equal": geometry_equal(
                        mesh_original, mesh_reversed
                    ),
                    "point_near_equal": geometry_equal(
                        point_original, point_near
                    ),
                    "polyline_reversed_equal": geometry_equal(
                        open_polyline, open_polyline_reversed
                    ),
                    "without_original_count": len(selected_without_originals),
                }

            return _measure(iterations, duplicate_selection_cycle)
        finally:
            for geometry in geometries:
                geometry.Dispose()

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

    if kind == "polyline_area":
        polyline = Rhino.Geometry.Polyline(
            [_point(vertex) for vertex in operation["vertices"]]
        )
        if not polyline.IsValid or not polyline.IsClosed:
            raise ValueError("area polyline must be valid and closed")
        curve = Rhino.Geometry.PolylineCurve(polyline)

        def polyline_area():
            properties = Rhino.Geometry.AreaMassProperties.Compute(curve)
            if properties is None:
                raise ValueError("could not compute polyline area")
            area = float(properties.Area)
            properties.Dispose()
            return area

        value, elapsed = _measure(iterations, polyline_area)
        return value, elapsed

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

    if kind == "nurbs_curve_closest_point":
        degree = int(operation["degree"])
        controls = operation["control_points"]
        curve = Rhino.Geometry.NurbsCurve(3, True, degree + 1, len(controls))
        _set_curve_controls(curve, controls)
        _set_knots(curve.Knots, operation["knots"], "curve knot")
        if not curve.IsValid:
            raise ValueError("NURBS curve is invalid")
        target = _point(operation["target"])

        def curve_closest_point():
            success, parameter = curve.ClosestPoint(target)
            if not success:
                raise ValueError("NURBS curve closest-point search failed")
            closest = curve.PointAt(parameter)
            return {
                "distance": float(closest.DistanceTo(target)),
                "parameter": float(parameter),
                "point": _xyz(closest),
            }

        return _measure(iterations, curve_closest_point)

    if kind == "nurbs_curve_length":
        degree = int(operation["degree"])
        controls = operation["control_points"]
        curve = Rhino.Geometry.NurbsCurve(3, True, degree + 1, len(controls))
        _set_curve_controls(curve, controls)
        _set_knots(curve.Knots, operation["knots"], "curve knot")
        if not curve.IsValid:
            raise ValueError("NURBS curve is invalid")
        value, elapsed = _measure(
            iterations, lambda: curve.GetLength(tolerance["relative"])
        )
        return float(value), elapsed

    if kind == "nurbs_curve_short_filter":
        degree = int(operation["degree"])
        controls = operation["control_points"]
        maximum_length = _finite(
            operation["maximum_length"], "maximum curve length"
        )
        if not maximum_length > 0.0:
            raise ValueError("maximum curve length must be strictly positive")
        curve = Rhino.Geometry.NurbsCurve(3, True, degree + 1, len(controls))
        _set_curve_controls(curve, controls)
        _set_knots(curve.Knots, operation["knots"], "curve knot")
        if not curve.IsValid:
            raise ValueError("NURBS curve is invalid")

        def curve_short_filter():
            return bool(
                curve.GetLength(tolerance["relative"]) <= maximum_length
            )

        return _measure(iterations, curve_short_filter)

    if kind == "nurbs_curve_topology":
        degree = int(operation["degree"])
        controls = operation["control_points"]
        curve = Rhino.Geometry.NurbsCurve(3, True, degree + 1, len(controls))
        _set_curve_controls(curve, controls)
        _set_knots(curve.Knots, operation["knots"], "curve knot")
        if not curve.IsValid:
            raise ValueError("NURBS curve is invalid")

        def curve_topology():
            return {
                "is_closed": bool(curve.IsClosed),
                "is_periodic": bool(curve.IsPeriodic),
            }

        return _measure(iterations, curve_topology)

    if kind == "nurbs_curve_classification":
        degree = int(operation["degree"])
        controls = operation["control_points"]
        curve = Rhino.Geometry.NurbsCurve(3, True, degree + 1, len(controls))
        _set_curve_controls(curve, controls)
        _set_knots(curve.Knots, operation["knots"], "curve knot")
        if not curve.IsValid:
            raise ValueError("NURBS curve is invalid")

        def curve_classification():
            is_linear_zero = bool(curve.IsLinear())
            return {
                "is_linear_model": bool(
                    curve.IsLinear(tolerance["absolute"])
                ),
                "is_linear_zero": is_linear_zero,
                "is_planar_model": bool(
                    curve.IsPlanar(tolerance["absolute"])
                ),
                "sel_line_match": bool(
                    curve.SpanCount == 1 and is_linear_zero
                ),
                "sel_polyline_match": bool(
                    curve.Degree == 1 and curve.Points.Count > 2
                ),
            }

        return _measure(iterations, curve_classification)

    if kind == "nurbs_curve_extract_points":
        degree = int(operation["degree"])
        controls = operation["control_points"]
        curve = Rhino.Geometry.NurbsCurve(3, True, degree + 1, len(controls))
        _set_curve_controls(curve, controls)
        _set_knots(curve.Knots, operation["knots"], "curve knot")
        if not curve.IsValid:
            raise ValueError("NURBS curve is invalid")
        document = Rhino.RhinoDoc.ActiveDoc
        object_id = document.Objects.AddCurve(curve)
        if object_id == System.Guid.Empty:
            raise ValueError("could not add NURBS curve grip probe")
        curve_object = document.Objects.FindId(object_id)
        try:
            curve_object.GripsOn = True
            grips = curve_object.GetGrips()
            if grips is None:
                raise ValueError("could not enable NURBS curve grips")
            return _measure(
                iterations,
                lambda: [_xyz(grip.CurrentLocation) for grip in grips],
            )
        finally:
            curve_object.GripsOn = False
            document.Objects.Delete(object_id, True)

    if kind == "nurbs_curve_divide":
        degree = int(operation["degree"])
        controls = operation["control_points"]
        curve = Rhino.Geometry.NurbsCurve(3, True, degree + 1, len(controls))
        _set_curve_controls(curve, controls)
        _set_knots(curve.Knots, operation["knots"], "curve knot")
        if not curve.IsValid:
            raise ValueError("NURBS curve is invalid")
        segment_count = int(operation["segment_count"])
        include_ends = bool(operation["include_ends"])
        first_index = 0 if include_ends else 1
        last_index = segment_count if include_ends and not curve.IsClosed else segment_count - 1
        fractions = System.Array[System.Double](
            [
                float(index) / float(segment_count)
                for index in iteration_range(first_index, last_index + 1)
            ]
        )
        default_parameters = curve.DivideByCount(segment_count, include_ends)
        if default_parameters is None or len(default_parameters) != len(fractions):
            raise ValueError("NURBS curve division returned an unexpected point count")

        def divide_curve():
            parameters = []
            for fraction in fractions:
                # DivideByCount uses RhinoCommon's fixed 1e-8 fractional
                # tolerance. Its point count is checked above; use the public
                # tolerance-bearing solver for coordinate comparisons and
                # leave margin for the external epsilon check.
                success, parameter = curve.NormalizedLengthParameter(
                    fraction, tolerance["relative"] * 0.001
                )
                if not success:
                    raise ValueError("NURBS curve division failed")
                parameters.append(parameter)
            return [_xyz(curve.PointAt(parameter)) for parameter in parameters]

        return _measure(iterations, divide_curve)

    if kind == "nurbs_curve_reverse":
        degree = int(operation["degree"])
        controls = operation["control_points"]
        curve = Rhino.Geometry.NurbsCurve(3, True, degree + 1, len(controls))
        _set_curve_controls(curve, controls)
        _set_knots(curve.Knots, operation["knots"], "curve knot")
        if not curve.IsValid:
            raise ValueError("NURBS curve is invalid")
        normalized = _finite(
            operation["normalized_parameter"], "normalized curve parameter"
        )
        if normalized < 0.0 or normalized > 1.0:
            raise ValueError("normalized curve parameter must be in [0, 1]")

        def reverse_curve():
            reversed_curve = curve.DuplicateCurve()
            if reversed_curve is None:
                raise ValueError("could not duplicate NURBS curve")
            try:
                if not reversed_curve.Reverse():
                    raise ValueError("could not reverse NURBS curve")
                parameter = reversed_curve.Domain.ParameterAt(normalized)
                values = reversed_curve.DerivativeAt(parameter, 1)
                if values is None or len(values) < 2:
                    raise ValueError("reversed NURBS curve evaluation failed")
                return {"point": _xyz(values[0]), "derivative": _xyz(values[1])}
            finally:
                reversed_curve.Dispose()

        return _measure(iterations, reverse_curve)

    if kind == "mesh_unify_normals":
        source = _triangle_mesh(operation["vertices"], operation["triangles"])

        def unify_mesh_normals():
            mesh = source.DuplicateMesh()
            if mesh is None:
                raise ValueError("could not duplicate mesh")
            try:
                flipped_faces = int(mesh.UnifyNormals())
                if flipped_faces < 0:
                    raise ValueError("mesh face unification failed")
                return {
                    "flipped_faces": flipped_faces,
                    "triangles": _mesh_triangles(mesh),
                }
            finally:
                mesh.Dispose()

        try:
            return _measure(iterations, unify_mesh_normals)
        finally:
            source.Dispose()

    if kind == "mesh_disjoint_pieces":
        source = _triangle_mesh(operation["vertices"], operation["triangles"])

        def split_disjoint_mesh():
            pieces = source.SplitDisjointPieces()
            if pieces is None:
                raise ValueError("mesh disjoint split failed")
            try:
                return {
                    "disjoint_mesh_count": int(source.DisjointMeshCount),
                    "pieces": [
                        _mesh_value(piece) for piece in pieces
                    ],
                }
            finally:
                for piece in pieces:
                    piece.Dispose()

        try:
            return _measure(iterations, split_disjoint_mesh)
        finally:
            source.Dispose()

    if kind == "mesh_combine_identical_vertices":
        source = _triangle_mesh(operation["vertices"], operation["triangles"])
        def combine_identical_vertices():
            mesh = source.DuplicateMesh()
            if mesh is None:
                raise ValueError("could not duplicate mesh")
            try:
                before = int(mesh.Vertices.Count)
                changed = bool(mesh.Vertices.CombineIdentical(True, True))
                return {
                    "changed": changed,
                    "removed_vertices": before - int(mesh.Vertices.Count),
                    "mesh": _mesh_value(mesh),
                }
            finally:
                mesh.Dispose()
        try:
            return _measure(iterations, combine_identical_vertices)
        finally:
            source.Dispose()

    if kind == "mesh_weld":
        source = _triangle_mesh(operation["vertices"], operation["triangles"])
        angle_radians = _finite(operation["angle_radians"], "mesh weld angle")
        def weld_mesh():
            mesh = source.DuplicateMesh()
            if mesh is None:
                raise ValueError("could not duplicate mesh")
            try:
                before = int(mesh.Vertices.Count)
                mesh.Weld(angle_radians)
                return {
                    "removed_vertices": before - int(mesh.Vertices.Count),
                    "mesh": _mesh_value(mesh),
                }
            finally:
                mesh.Dispose()
        try:
            return _measure(iterations, weld_mesh)
        finally:
            source.Dispose()

    if kind == "mesh_unweld":
        source = _triangle_mesh(operation["vertices"], operation["triangles"])
        angle_radians = _finite(operation["angle_radians"], "mesh unweld angle")
        modify_normals = operation["modify_normals"]
        if not isinstance(modify_normals, bool):
            source.Dispose()
            raise ValueError("mesh unweld modify_normals must be a boolean")
        def unweld_mesh():
            mesh = source.DuplicateMesh()
            if mesh is None:
                raise ValueError("could not duplicate mesh")
            try:
                before = int(mesh.Vertices.Count)
                mesh.Unweld(angle_radians, modify_normals)
                return {
                    "added_vertices": int(mesh.Vertices.Count) - before,
                    "mesh": _mesh_value(mesh),
                }
            finally:
                mesh.Dispose()
        try:
            return _measure(iterations, unweld_mesh)
        finally:
            source.Dispose()

    if kind == "mesh_unweld_edge":
        source = _triangle_mesh(operation["vertices"], operation["triangles"])
        edge_indices = operation["edge_indices"]
        if not isinstance(edge_indices, list) or any(
            isinstance(index, bool) or int(index) != index
            for index in edge_indices
        ):
            source.Dispose()
            raise ValueError("mesh unweld edge indices must be integers")
        edge_indices = [int(index) for index in edge_indices]
        modify_normals = operation["modify_normals"]
        if not isinstance(modify_normals, bool):
            source.Dispose()
            raise ValueError("mesh unweld edge modify_normals must be a boolean")
        def unweld_mesh_edges():
            mesh = source.DuplicateMesh()
            if mesh is None:
                raise ValueError("could not duplicate mesh")
            try:
                before = int(mesh.Vertices.Count)
                accepted = bool(mesh.UnweldEdge(edge_indices, modify_normals))
                return {
                    "accepted": accepted,
                    "added_vertices": int(mesh.Vertices.Count) - before,
                    "mesh": _mesh_unweld_value(mesh),
                }
            finally:
                mesh.Dispose()
        try:
            return _measure(iterations, unweld_mesh_edges)
        finally:
            source.Dispose()

    if kind == "mesh_cull_unused_vertices":
        source = _triangle_mesh(operation["vertices"], operation["triangles"])
        def cull_unused_vertices():
            mesh = source.DuplicateMesh()
            if mesh is None:
                raise ValueError("could not duplicate mesh")
            try:
                removed_vertices = int(mesh.Vertices.CullUnused())
                if removed_vertices < 0:
                    raise ValueError("mesh vertex culling failed")
                return {
                    "changed": removed_vertices > 0,
                    "removed_vertices": removed_vertices,
                    "mesh": _mesh_value(mesh),
                }
            finally:
                mesh.Dispose()
        try:
            return _measure(iterations, cull_unused_vertices)
        finally:
            source.Dispose()

    if kind == "mesh_volume":
        source = _triangle_mesh(operation["vertices"], operation["triangles"])
        try:
            return _measure(iterations, lambda: float(source.Volume()))
        finally:
            source.Dispose()

    if kind == "mesh_extract_duplicate_faces":
        source = _triangle_mesh(operation["vertices"], operation["triangles"])

        def extract_duplicate_faces():
            remainder = source.DuplicateMesh()
            if remainder is None:
                raise ValueError("could not duplicate mesh")
            extracted = None
            try:
                extracted = remainder.Faces.ExtractDuplicateFaces()
                return {
                    "extracted": (
                        None if extracted is None else _mesh_value(extracted)
                    ),
                    "remainder": _mesh_value(remainder),
                }
            finally:
                if extracted is not None:
                    extracted.Dispose()
                remainder.Dispose()

        try:
            return _measure(iterations, extract_duplicate_faces)
        finally:
            source.Dispose()

    if kind == "mesh_extract_faces":
        source = _triangle_mesh(operation["vertices"], operation["triangles"])
        face_indices = operation["face_indices"]
        if not isinstance(face_indices, list) or any(
            isinstance(index, bool) or int(index) != index
            for index in face_indices
        ):
            source.Dispose()
            raise ValueError("mesh extraction face indices must be integers")
        face_indices = [int(index) for index in face_indices]

        def extract_faces():
            remainder = source.DuplicateMesh()
            if remainder is None:
                raise ValueError("could not duplicate mesh")
            extracted = None
            try:
                extracted = remainder.Faces.ExtractFaces(face_indices)
                if extracted is None:
                    raise ValueError("mesh face extraction failed")
                return {
                    "extracted": _mesh_value(extracted),
                    "remainder": (
                        None if remainder.Faces.Count == 0 else _mesh_value(remainder)
                    ),
                }
            finally:
                if extracted is not None:
                    extracted.Dispose()
                remainder.Dispose()

        try:
            return _measure(iterations, extract_faces)
        finally:
            source.Dispose()

    if kind == "mesh_delete_faces":
        source = _triangle_mesh(operation["vertices"], operation["triangles"])
        face_indices = operation["face_indices"]
        if not isinstance(face_indices, list) or any(
            isinstance(index, bool) or int(index) != index
            for index in face_indices
        ):
            source.Dispose()
            raise ValueError("mesh deletion face indices must be integers")
        face_indices = [int(index) for index in face_indices]

        def delete_faces():
            remainder = source.DuplicateMesh()
            if remainder is None:
                raise ValueError("could not duplicate mesh")
            try:
                deleted_face_count = int(
                    remainder.Faces.DeleteFaces(face_indices, True)
                )
                if deleted_face_count != len(face_indices):
                    raise ValueError("mesh face deletion failed")
                return {
                    "deleted_face_count": deleted_face_count,
                    "remainder": (
                        None if remainder.Faces.Count == 0 else _mesh_value(remainder)
                    ),
                }
            finally:
                remainder.Dispose()

        try:
            return _measure(iterations, delete_faces)
        finally:
            source.Dispose()

    if kind == "mesh_triangulate":
        source = _polygon_mesh(operation["vertices"], operation["faces"])

        def triangulate_mesh():
            mesh = source.DuplicateMesh()
            if mesh is None:
                raise ValueError("could not duplicate mesh")
            try:
                before = int(mesh.Faces.QuadCount)
                if not mesh.Faces.ConvertQuadsToTriangles():
                    raise ValueError("mesh triangulation failed")
                converted = before - int(mesh.Faces.QuadCount)
                return {
                    "converted_quad_count": converted,
                    "mesh": _polygon_mesh_value(mesh),
                }
            finally:
                mesh.Dispose()

        try:
            return _measure(iterations, triangulate_mesh)
        finally:
            source.Dispose()

    if kind == "mesh_swap_edge":
        source = _polygon_mesh(operation["vertices"], operation["faces"])
        edge_points = [_point(point) for point in operation["edge_points"]]
        if len(edge_points) != 2:
            source.Dispose()
            raise ValueError("mesh swap edge requires two endpoint locations")
        topology_edge_index = -1
        for edge_index in range(source.TopologyEdges.Count):
            line = source.TopologyEdges.EdgeLine(edge_index)
            if (
                (line.From == edge_points[0] and line.To == edge_points[1])
                or (line.From == edge_points[1] and line.To == edge_points[0])
            ):
                topology_edge_index = edge_index
                break
        if topology_edge_index < 0:
            source.Dispose()
            raise ValueError("mesh swap edge endpoints do not identify an edge")

        def swap_mesh_edge():
            mesh = source.DuplicateMesh()
            if mesh is None:
                raise ValueError("could not duplicate mesh")
            try:
                accepted = bool(mesh.TopologyEdges.SwapEdge(topology_edge_index))
                return {
                    "accepted": accepted,
                    "mesh": _polygon_mesh_value(mesh),
                }
            finally:
                mesh.Dispose()

        try:
            return _measure(iterations, swap_mesh_edge)
        finally:
            source.Dispose()

    if kind == "mesh_collapse_edge":
        source = _polygon_mesh(operation["vertices"], operation["faces"])
        edge_points = [_point(point) for point in operation["edge_points"]]
        if len(edge_points) != 2:
            source.Dispose()
            raise ValueError("mesh collapse edge requires two endpoint locations")
        topology_edge_index = -1
        for edge_index in range(source.TopologyEdges.Count):
            line = source.TopologyEdges.EdgeLine(edge_index)
            if (
                (line.From == edge_points[0] and line.To == edge_points[1])
                or (line.From == edge_points[1] and line.To == edge_points[0])
            ):
                topology_edge_index = edge_index
                break
        if topology_edge_index < 0:
            source.Dispose()
            raise ValueError("mesh collapse edge endpoints do not identify an edge")

        def collapse_mesh_edge():
            mesh = source.DuplicateMesh()
            if mesh is None:
                raise ValueError("could not duplicate mesh")
            try:
                accepted = bool(mesh.TopologyEdges.CollapseEdge(topology_edge_index))
                return {
                    "accepted": accepted,
                    "mesh": (
                        None
                        if mesh.Faces.Count == 0
                        else _polygon_mesh_value(mesh)
                    ),
                }
            finally:
                mesh.Dispose()

        try:
            return _measure(iterations, collapse_mesh_edge)
        finally:
            source.Dispose()

    if kind == "mesh_split_edge":
        source = _polygon_mesh(operation["vertices"], operation["faces"])
        edge_points = [_point(point) for point in operation["edge_points"]]
        if len(edge_points) != 2:
            source.Dispose()
            raise ValueError("mesh split edge requires two endpoint locations")
        topology_edge_index = -1
        for edge_index in range(source.TopologyEdges.Count):
            line = source.TopologyEdges.EdgeLine(edge_index)
            if (
                (line.From == edge_points[0] and line.To == edge_points[1])
                or (line.From == edge_points[1] and line.To == edge_points[0])
            ):
                topology_edge_index = edge_index
                break
        if topology_edge_index < 0:
            source.Dispose()
            raise ValueError("mesh split edge endpoints do not identify an edge")

        def split_mesh_edge():
            mesh = source.DuplicateMesh()
            if mesh is None:
                raise ValueError("could not duplicate mesh")
            try:
                accepted = bool(
                    mesh.TopologyEdges.SplitEdge(
                        topology_edge_index, float(operation["parameter"])
                    )
                )
                return {
                    "accepted": accepted,
                    "mesh": _polygon_mesh_value(mesh),
                }
            finally:
                mesh.Dispose()

        try:
            return _measure(iterations, split_mesh_edge)
        finally:
            source.Dispose()

    if kind == "mesh_fill_hole":
        source = _polygon_mesh(operation["vertices"], operation["faces"])
        edge_points = [_point(point) for point in operation["edge_points"]]
        if len(edge_points) != 2:
            source.Dispose()
            raise ValueError("mesh fill hole requires two endpoint locations")
        topology_edge_index = -1
        for edge_index in range(source.TopologyEdges.Count):
            line = source.TopologyEdges.EdgeLine(edge_index)
            if (
                (line.From == edge_points[0] and line.To == edge_points[1])
                or (line.From == edge_points[1] and line.To == edge_points[0])
            ):
                topology_edge_index = edge_index
                break
        if topology_edge_index < 0:
            source.Dispose()
            raise ValueError("mesh fill hole endpoints do not identify an edge")

        def fill_mesh_hole():
            mesh = source.DuplicateMesh()
            if mesh is None:
                raise ValueError("could not duplicate mesh")
            try:
                # Rhino 8's public binding exposes the historical spelling;
                # current RhinoCommon documentation aliases it as FillHole.
                accepted = bool(mesh.FileHole(topology_edge_index))
                return {
                    "accepted": accepted,
                    "mesh": _mesh_fill_hole_value(
                        mesh, source.Vertices.Count, source.Faces.Count
                    ),
                }
            finally:
                mesh.Dispose()

        try:
            return _measure(iterations, fill_mesh_hole)
        finally:
            source.Dispose()

    if kind == "mesh_fill_holes":
        source = _polygon_mesh(operation["vertices"], operation["faces"])

        def fill_mesh_holes():
            mesh = source.DuplicateMesh()
            if mesh is None:
                raise ValueError("could not duplicate mesh")
            try:
                accepted = bool(mesh.FillHoles())
                return {
                    "accepted": accepted,
                    "mesh": _mesh_fill_hole_value(
                        mesh, source.Vertices.Count, source.Faces.Count
                    ),
                }
            finally:
                mesh.Dispose()

        try:
            return _measure(iterations, fill_mesh_holes)
        finally:
            source.Dispose()

    if kind == "mesh_extract_non_manifold":
        source = _triangle_mesh(operation["vertices"], operation["triangles"])
        selective = operation["selective"]
        if not isinstance(selective, bool):
            source.Dispose()
            raise ValueError("mesh extraction selective flag must be boolean")

        def extract_non_manifold():
            remainder = source.DuplicateMesh()
            if remainder is None:
                raise ValueError("could not duplicate mesh")
            extracted = None
            try:
                extracted = remainder.ExtractNonManifoldEdges(selective)
                return {
                    "extracted": (
                        None if extracted is None else _mesh_value(extracted)
                    ),
                    "remainder": (
                        None if remainder.Faces.Count == 0 else _mesh_value(remainder)
                    ),
                }
            finally:
                if extracted is not None:
                    extracted.Dispose()
                remainder.Dispose()

        try:
            return _measure(iterations, extract_non_manifold)
        finally:
            source.Dispose()

    if kind == "mesh_to_nurb":
        source = _polygon_mesh(operation["vertices"], operation["faces"])
        trimmed = bool(operation["trim_triangular_faces"])

        def convert_mesh():
            brep = Rhino.Geometry.Brep.CreateFromMesh(source, trimmed)
            if brep is None:
                raise ValueError("could not convert mesh to B-rep")
            try:
                return _mesh_to_nurb_brep_value(brep)
            finally:
                brep.Dispose()

        try:
            return _measure(iterations, convert_mesh)
        finally:
            source.Dispose()

    if kind == "mesh_plane":
        plane = Rhino.Geometry.Plane(
            _point(operation["origin"]),
            _vector(operation["x_axis"]),
            _vector(operation["y_axis"]),
        )
        x_interval = Rhino.Geometry.Interval(
            _finite(operation["x_interval"][0], "mesh-plane x interval"),
            _finite(operation["x_interval"][1], "mesh-plane x interval"),
        )
        y_interval = Rhino.Geometry.Interval(
            _finite(operation["y_interval"][0], "mesh-plane y interval"),
            _finite(operation["y_interval"][1], "mesh-plane y interval"),
        )
        x_count = int(operation["x_count"])
        y_count = int(operation["y_count"])

        def create_mesh_plane():
            mesh = Rhino.Geometry.Mesh.CreateFromPlane(
                plane, x_interval, y_interval, x_count, y_count
            )
            if mesh is None:
                raise ValueError("could not create mesh plane")
            try:
                return _polygon_mesh_value(mesh)
            finally:
                mesh.Dispose()

        return _measure(iterations, create_mesh_plane)

    if kind == "mesh_box":
        plane = Rhino.Geometry.Plane(
            _point(operation["origin"]),
            _vector(operation["x_axis"]),
            _vector(operation["y_axis"]),
        )
        intervals = [
            Rhino.Geometry.Interval(
                _finite(operation[name][0], "mesh-box interval"),
                _finite(operation[name][1], "mesh-box interval"),
            )
            for name in ("x_interval", "y_interval", "z_interval")
        ]
        box = Rhino.Geometry.Box(plane, intervals[0], intervals[1], intervals[2])
        counts = [
            int(operation[name]) for name in ("x_count", "y_count", "z_count")
        ]

        def create_mesh_box():
            mesh = Rhino.Geometry.Mesh.CreateFromBox(
                box, counts[0], counts[1], counts[2]
            )
            if mesh is None:
                raise ValueError("could not create mesh box")
            try:
                return _polygon_mesh_value(mesh)
            finally:
                mesh.Dispose()

        return _measure(iterations, create_mesh_box)

    if kind == "mesh_cylinder":
        plane = Rhino.Geometry.Plane(
            _point(operation["origin"]),
            _vector(operation["x_axis"]),
            _vector(operation["y_axis"]),
        )
        radius = _finite(operation["radius"], "mesh-cylinder radius")
        heights = [
            _finite(value, "mesh-cylinder height")
            for value in operation["heights"]
        ]
        base_origin = plane.Origin + heights[0] * plane.ZAxis
        base_plane = Rhino.Geometry.Plane(base_origin, plane.XAxis, plane.YAxis)
        circle = Rhino.Geometry.Circle(base_plane, radius)
        cylinder = Rhino.Geometry.Cylinder(circle, heights[1] - heights[0])
        vertical = int(operation["vertical"])
        around = int(operation["around"])
        cap_bottom = bool(operation["cap_bottom"])
        cap_top = bool(operation["cap_top"])
        circumscribe = bool(operation["circumscribe"])
        quad_caps = bool(operation["quad_caps"])

        def create_mesh_cylinder():
            mesh = Rhino.Geometry.Mesh.CreateFromCylinder(
                cylinder,
                vertical,
                around,
                # Rhino 8.32's Python binding exposes these two positional
                # booleans in physical top-then-bottom order.
                cap_top,
                cap_bottom,
                circumscribe,
                quad_caps,
            )
            if mesh is None:
                raise ValueError("could not create mesh cylinder")
            try:
                return _polygon_mesh_value(mesh)
            finally:
                mesh.Dispose()

        return _measure(iterations, create_mesh_cylinder)

    if kind == "mesh_cone":
        plane = Rhino.Geometry.Plane(
            _point(operation["origin"]),
            _vector(operation["x_axis"]),
            _vector(operation["y_axis"]),
        )
        radius = _finite(operation["radius"], "mesh-cone radius")
        height_to_base = _finite(
            operation["height_to_base"], "mesh-cone height"
        )
        cone = Rhino.Geometry.Cone(plane, height_to_base, radius)
        vertical = int(operation["vertical"])
        around = int(operation["around"])
        solid = bool(operation["solid"])
        quad_caps = bool(operation["quad_caps"])

        def create_mesh_cone():
            mesh = Rhino.Geometry.Mesh.CreateFromCone(
                cone,
                vertical,
                around,
                solid,
                quad_caps,
            )
            if mesh is None:
                raise ValueError("could not create mesh cone")
            try:
                return _polygon_mesh_value(mesh)
            finally:
                mesh.Dispose()

        return _measure(iterations, create_mesh_cone)

    if kind == "mesh_sphere":
        plane = Rhino.Geometry.Plane(
            _point(operation["origin"]),
            _vector(operation["x_axis"]),
            _vector(operation["y_axis"]),
        )
        radius = _finite(operation["radius"], "mesh-sphere radius")
        sphere = Rhino.Geometry.Sphere(plane, radius)
        around = int(operation["around"])
        vertical = int(operation["vertical"])

        def create_mesh_sphere():
            mesh = Rhino.Geometry.Mesh.CreateFromSphere(
                sphere,
                around,
                vertical,
            )
            if mesh is None:
                raise ValueError("could not create mesh sphere")
            try:
                return _polygon_mesh_value(mesh)
            finally:
                mesh.Dispose()

        return _measure(iterations, create_mesh_sphere)

    if kind == "mesh_ellipsoid":
        document = Rhino.RhinoDoc.ActiveDoc
        plane = Rhino.Geometry.Plane(
            _point(operation["origin"]),
            _vector(operation["x_axis"]),
            _vector(operation["y_axis"]),
        )
        radii = [
            _finite(value, "mesh-ellipsoid radius")
            for value in operation["radii"]
        ]
        vertical = int(operation["vertical"])
        around = int(operation["around"])
        quad_caps = bool(operation["quad_caps"])

        def point_text(point):
            return "%.17g,%.17g,%.17g" % (point.X, point.Y, point.Z)

        center = plane.Origin
        first_axis = center + radii[0] * plane.XAxis
        second_axis = center + radii[1] * plane.YAxis
        third_axis = center + radii[2] * plane.ZAxis
        command = (
            "_-MeshEllipsoid _VerticalFaces=%d _AroundFaces=4 "
            "_CapFaceStyle=_%s _AroundFaces=%d %s %s %s %s"
            % (
                vertical,
                "Quad" if quad_caps else "Tri",
                around,
                point_text(center),
                point_text(first_axis),
                point_text(second_axis),
                point_text(third_axis),
            )
        )

        def create_mesh_ellipsoid():
            before = set(obj.Id for obj in document.Objects)
            document.Objects.UnselectAll()
            succeeded = Rhino.RhinoApp.RunScript(command, False)
            created = [obj for obj in document.Objects if obj.Id not in before]
            try:
                meshes = [
                    obj.Geometry
                    for obj in created
                    if isinstance(obj.Geometry, Rhino.Geometry.Mesh)
                ]
                if len(meshes) != 1:
                    history = Rhino.RhinoApp.CommandHistoryWindowText
                    raise ValueError(
                        "MeshEllipsoid macro %r returned %r and created %d meshes; history tail: %s"
                        % (command, succeeded, len(meshes), history[-2000:])
                    )
                return _polygon_mesh_value(meshes[0])
            finally:
                for obj in created:
                    document.Objects.Delete(obj.Id, True)

        return _measure(iterations, create_mesh_ellipsoid)

    if kind == "mesh_truncated_cone":
        document = Rhino.RhinoDoc.ActiveDoc
        plane = Rhino.Geometry.Plane(
            _point(operation["origin"]),
            _vector(operation["x_axis"]),
            _vector(operation["y_axis"]),
        )
        base_radius = _finite(
            operation["base_radius"], "mesh truncated-cone base radius"
        )
        end_radius = _finite(
            operation["end_radius"], "mesh truncated-cone end radius"
        )
        height = _finite(operation["height"], "mesh truncated-cone height")
        vertical = int(operation["vertical"])
        around = int(operation["around"])
        solid = bool(operation["solid"])
        quad_caps = bool(operation["quad_caps"])
        option_text = "_VerticalFaces=%d _AroundFaces=%d _Solid=_No" % (
            vertical,
            around,
        )
        if solid:
            # CapFaceStyle is offered only while AroundFaces is even. Set a
            # temporary even count, choose the style, then restore the target;
            # Rhino itself falls back to triangle caps for odd target counts.
            option_text = (
                "_VerticalFaces=%d _AroundFaces=4 _Solid=_Yes "
                "_CapFaceStyle=_%s _AroundFaces=%d"
                % (
                    vertical,
                    "Quad" if quad_caps else "Tri",
                    around,
                )
            )
        command = "_-MeshTruncatedCone %s 0,0,0 %.17g %.17g %.17g" % (
            option_text,
            base_radius,
            height,
            end_radius,
        )
        transform = Rhino.Geometry.Transform.PlaneToPlane(
            Rhino.Geometry.Plane.WorldXY, plane
        )

        def create_mesh_truncated_cone():
            before = set(obj.Id for obj in document.Objects)
            document.Objects.UnselectAll()
            succeeded = Rhino.RhinoApp.RunScript(command, False)
            created = [obj for obj in document.Objects if obj.Id not in before]
            try:
                meshes = [
                    obj.Geometry
                    for obj in created
                    if isinstance(obj.Geometry, Rhino.Geometry.Mesh)
                ]
                if len(meshes) != 1:
                    history = Rhino.RhinoApp.CommandHistoryWindowText
                    raise ValueError(
                        "MeshTruncatedCone macro %r returned %r and created %d meshes; history tail: %s"
                        % (command, succeeded, len(meshes), history[-2000:])
                    )
                mesh = meshes[0].DuplicateMesh()
                if mesh is None:
                    raise ValueError("could not duplicate mesh truncated cone")
                try:
                    if not mesh.Transform(transform):
                        raise ValueError("could not orient mesh truncated cone")
                    return _polygon_mesh_value(mesh)
                finally:
                    mesh.Dispose()
            finally:
                for obj in created:
                    document.Objects.Delete(obj.Id, True)

        return _measure(iterations, create_mesh_truncated_cone)

    if kind == "parabola":
        document = Rhino.RhinoDoc.ActiveDoc
        plane = Rhino.Geometry.Plane(
            _point(operation["origin"]),
            _vector(operation["x_axis"]),
            _vector(operation["y_axis"]),
        )
        transform = Rhino.Geometry.Transform.PlaneToPlane(
            Rhino.Geometry.Plane.WorldXY, plane
        )
        radius = _finite(operation["radius"], "parabola radius")
        height = _finite(operation["height"], "parabola height")
        if not radius > 0.0 or not height > 0.0:
            raise ValueError("parabola dimensions must be positive")
        focal_distance = 0.25 * radius * (radius / height)
        if math.isnan(focal_distance) or math.isinf(focal_distance):
            raise ValueError("parabola focal distance must be finite")
        half = bool(operation["half"])
        command = (
            "_-Parabola _Vertex _MarkFocus=_No _Half=_%s "
            "0,0,0 0,0,%.17g %.17g,0,0"
            % ("Yes" if half else "No", focal_distance, radius)
        )

        def create_parabola():
            before = set(obj.Id for obj in document.Objects)
            document.Objects.UnselectAll()
            succeeded = Rhino.RhinoApp.RunScript(command, False)
            created = [obj for obj in document.Objects if obj.Id not in before]
            curve = None
            try:
                if len(created) != 1:
                    history = Rhino.RhinoApp.CommandHistoryWindowText
                    raise ValueError(
                        "parabola macro %r returned %r and created %d objects; "
                        "history tail: %s"
                        % (command, succeeded, len(created), history[-3000:])
                    )
                geometry = created[0].Geometry
                if isinstance(geometry, Rhino.Geometry.Curve):
                    curve = geometry.DuplicateCurve()
                if curve is None:
                    raise ValueError("parabola did not create curve geometry")
                if not curve.Transform(transform):
                    raise ValueError("could not orient parabola")
                return _nurbs_curve_definition(curve)
            finally:
                if curve is not None:
                    curve.Dispose()
                for obj in created:
                    document.Objects.Delete(obj.Id, True)

        return _measure(iterations, create_parabola)

    if kind == "parabola_three_point":
        document = Rhino.RhinoDoc.ActiveDoc
        mode = str(operation["mode"])
        mode_option = {
            "focus": "Focus",
            "through_point": "ThroughPoint",
            "vertex": "Vertex",
        }.get(mode)
        if mode_option is None:
            raise ValueError("unknown three-point parabola mode: %s" % mode)
        start = _point(operation["start"])
        special = _point(operation["special"])
        end = _point(operation["end"])
        command = (
            "_-Parabola3Pt _Mode=_%s _PickOrder=_EndsFirst _MarkFocus=_No "
            "%s %s %s"
            % (
                mode_option,
                _command_point(_xyz(start)),
                _command_point(_xyz(end)),
                _command_point(_xyz(special)),
            )
        )
        if mode == "through_point":
            direction = _vector(operation.get("opening_direction"))
            _unit(
                Rhino.Geometry.Vector3d(direction),
                tolerance["absolute"],
                "three-point parabola opening direction",
            )
            direction_point = special + direction
            command += " " + _command_point(_xyz(direction_point))

        def create_parabola_three_point():
            before = set(obj.Id for obj in document.Objects)
            document.Objects.UnselectAll()
            succeeded = Rhino.RhinoApp.RunScript(command, False)
            created = [obj for obj in document.Objects if obj.Id not in before]
            curve = None
            try:
                if len(created) != 1:
                    history = Rhino.RhinoApp.CommandHistoryWindowText
                    raise ValueError(
                        "three-point parabola macro %r returned %r and created "
                        "%d objects; history tail: %s"
                        % (command, succeeded, len(created), history[-3000:])
                    )
                geometry = created[0].Geometry
                if isinstance(geometry, Rhino.Geometry.Curve):
                    curve = geometry.DuplicateCurve()
                if curve is None:
                    raise ValueError(
                        "three-point parabola did not create curve geometry"
                    )
                return _nurbs_curve_definition(curve)
            finally:
                if curve is not None:
                    curve.Dispose()
                for obj in created:
                    document.Objects.Delete(obj.Id, True)

        return _measure(iterations, create_parabola_three_point)

    if kind == "helix":
        origin = _point(operation["origin"])
        plane = Rhino.Geometry.Plane(
            origin,
            _vector(operation["x_axis"]),
            _vector(operation["y_axis"]),
        )
        radius = _finite(operation["radius"], "helix radius")
        height = _finite(operation["height"], "helix height")
        turns = _finite(operation["turns"], "helix turns")
        if not radius > 0.0 or not height > 0.0 or turns == 0.0:
            raise ValueError("helix dimensions must be positive and nonzero")
        radius_point = origin + radius * plane.XAxis
        pitch = height / abs(turns)

        def create_helix():
            curve = Rhino.Geometry.NurbsCurve.CreateSpiral(
                origin,
                plane.ZAxis,
                radius_point,
                pitch,
                turns,
                radius,
                radius,
            )
            if curve is None:
                raise ValueError("Rhino could not create helix")
            try:
                curve.Domain = Rhino.Geometry.Interval(0.0, abs(turns))
                return _nurbs_curve_definition(curve)
            finally:
                curve.Dispose()

        return _measure(iterations, create_helix)

    if kind == "spiral":
        origin = _point(operation["origin"])
        plane = Rhino.Geometry.Plane(
            origin,
            _vector(operation["x_axis"]),
            _vector(operation["y_axis"]),
        )
        height = _finite(operation["height"], "spiral height")
        turns = _finite(operation["turns"], "spiral turns")
        radii = [
            _finite(value, "spiral radius") for value in operation["radii"]
        ]
        if turns == 0.0 or (radii[0] == 0.0 and radii[1] == 0.0):
            raise ValueError("spiral requires turns and at least one nonzero radius")
        radius_point = origin + plane.XAxis
        pitch = height / abs(turns)

        def create_spiral():
            curve = Rhino.Geometry.NurbsCurve.CreateSpiral(
                origin,
                plane.ZAxis,
                radius_point,
                pitch,
                turns,
                radii[0],
                radii[1],
            )
            if curve is None:
                raise ValueError("Rhino could not create spiral")
            try:
                curve.Domain = Rhino.Geometry.Interval(0.0, abs(turns))
                return _nurbs_curve_definition(curve)
            finally:
                curve.Dispose()

        return _measure(iterations, create_spiral)

    if kind == "swept_spiral":
        degree = int(operation["rail_degree"])
        controls = operation["rail_control_points"]
        rail = Rhino.Geometry.NurbsCurve(3, True, degree + 1, len(controls))
        _set_curve_controls(rail, controls)
        _set_knots(rail.Knots, operation["rail_knots"], "rail knot")
        if not rail.IsValid:
            raise ValueError("swept-spiral rail is invalid")
        radius_point = _point(operation["radius_point"])
        turns = _finite(operation["turns"], "swept-spiral turns")
        if turns == 0.0:
            raise ValueError("swept spiral requires a nonzero turn count")
        pitch = rail.GetLength() / abs(turns)
        if turns < 0.0:
            pitch = -pitch
        radii = [
            _finite(value, "swept-spiral radius")
            for value in operation["radii"]
        ]
        points_per_turn = int(operation["points_per_turn"])

        def create_swept_spiral():
            curve = Rhino.Geometry.NurbsCurve.CreateSpiral(
                rail,
                rail.Domain.T0,
                rail.Domain.T1,
                radius_point,
                pitch,
                turns,
                radii[0],
                radii[1],
                points_per_turn,
            )
            if curve is None:
                raise ValueError("Rhino could not create swept spiral")
            try:
                return _nurbs_curve_definition(curve)
            finally:
                curve.Dispose()

        try:
            return _measure(iterations, create_swept_spiral)
        finally:
            rail.Dispose()

    if kind == "catenary":
        document = Rhino.RhinoDoc.ActiveDoc
        start = _point(operation["start"])
        end = _point(operation["end"])
        axis_direction = _vector(operation["axis_direction"])
        construction = operation["construction"]
        mode = construction["mode"]
        smooth = bool(operation["smooth"])
        point_count = int(operation["point_count"])

        axis_direction.Unitize()
        axis_point = Rhino.Geometry.Point3d(
            start.X + axis_direction.X,
            start.Y + axis_direction.Y,
            start.Z + axis_direction.Z,
        )
        if mode in ("through_point", "apex"):
            mode_value = _command_point(construction["point"])
        else:
            mode_value = "%.17g" % _finite(
                construction["value"], "catenary mode value"
            )
        mode_name = {
            "through_point": "ThroughPoint",
            "length": "Length",
            "parameter": "Parameter",
            "apex": "Apex",
        }.get(mode)
        if mode_name is None:
            raise ValueError("unknown catenary construction mode")
        command = (
            "_-Catenary %s %s %s _Output=_%s _PointCount=%d "
            "_MarkApex=_No _Mode=_%s %s"
            % (
                _command_point(_xyz(start)),
                _command_point(_xyz(end)),
                _command_point(_xyz(axis_point)),
                "Smooth" if smooth else "Polyline",
                point_count,
                mode_name,
                mode_value,
            )
        )

        def create_catenary():
            before = set(obj.Id for obj in document.Objects)
            document.Objects.UnselectAll()
            succeeded = Rhino.RhinoApp.RunScript(command, False)
            created = [obj for obj in document.Objects if obj.Id not in before]
            curve = None
            try:
                curves = [
                    obj.Geometry
                    for obj in created
                    if isinstance(obj.Geometry, Rhino.Geometry.Curve)
                ]
                if len(curves) != 1:
                    history = Rhino.RhinoApp.CommandHistoryWindowText
                    raise ValueError(
                        "catenary macro %r returned %r and created %d curves; "
                        "history tail: %s"
                        % (command, succeeded, len(curves), history[-3000:])
                    )
                curve = curves[0].DuplicateCurve()
                curve_type = curve.GetType().Name
                definition = _nurbs_curve_definition(curve)
                if curve_type == "PolylineCurve":
                    return {
                        "curve_type": curve_type,
                        "points": [
                            control["point"]
                            for control in definition["control_points"]
                        ],
                    }
                return {"curve": definition, "curve_type": curve_type}
            finally:
                if curve is not None:
                    curve.Dispose()
                for obj in created:
                    document.Objects.Delete(obj.Id, True)

        return _measure(iterations, create_catenary)

    if kind == "curve_through_geometry":
        document = Rhino.RhinoDoc.ActiveDoc
        source = operation["source"]
        point_sets = operation["point_sets"]
        degree = int(operation["degree"])
        curve_type = operation["curve_type"]
        knots = operation["knots"]
        closed = bool(operation.get("closed", False))
        if source not in ("points", "polylines"):
            raise ValueError("curve-through source must be points or polylines")
        if curve_type not in ("control_point", "interpolated"):
            raise ValueError("unknown curve-through curve type")
        knot_name = {
            "uniform": "Uniform",
            "chord": "Chord",
            "sqrt_chord": "SqrtChord",
        }.get(knots)
        if knot_name is None:
            raise ValueError("unknown curve-through knot style")
        curve_type_name = {
            "control_point": "ControlPoint",
            "interpolated": "Interpolated",
        }[curve_type]
        if source == "points":
            knot_option = " _Knots=_%s" % knot_name if curve_type == "interpolated" else ""
            command = (
                "_-CurveThroughPt _Degree=%d _CurveType=_%s%s _Closed=_%s _Enter"
                % (degree, curve_type_name, knot_option, "Yes" if closed else "No")
            )
        else:
            knot_option = " _Knots=_%s" % knot_name if curve_type == "interpolated" else ""
            command = (
                "_-CurveThroughPolyline _Degree=%d _CurveType=_%s%s "
                "_DeleteInput=_No _Enter"
                % (degree, curve_type_name, knot_option)
            )

        def create_curves_through_geometry():
            before_all = set(obj.Id for obj in document.Objects)
            source_ids = []
            try:
                if source == "points":
                    if len(point_sets) != 1:
                        raise ValueError("point source requires one point set")
                    for coordinates in point_sets[0]:
                        source_ids.append(document.Objects.AddPoint(_point(coordinates)))
                else:
                    for coordinates in point_sets:
                        polyline = Rhino.Geometry.Polyline(
                            [_point(point) for point in coordinates]
                        )
                        source_ids.append(document.Objects.AddPolyline(polyline))
                document.Objects.UnselectAll()
                for source_id in source_ids:
                    document.Objects.Select(source_id, True)
                before_output = set(obj.Id for obj in document.Objects)
                succeeded = Rhino.RhinoApp.RunScript(command, False)
                curves = [
                    obj
                    for obj in document.Objects
                    if obj.Id not in before_output
                    and isinstance(obj.Geometry, Rhino.Geometry.Curve)
                    and obj.Geometry.GetType().Name != "PolylineCurve"
                ]
                curves.sort(key=lambda obj: obj.RuntimeSerialNumber)
                if len(curves) != len(point_sets):
                    history = Rhino.RhinoApp.CommandHistoryWindowText
                    raise ValueError(
                        "curve-through macro %r returned %r and created %d curves; "
                        "history tail: %s"
                        % (command, succeeded, len(curves), history[-3000:])
                    )
                definitions = []
                for obj in curves:
                    definition = _nurbs_curve_definition(obj.Geometry)
                    definition["knots"] = definition["knots"][1:-1]
                    definition["closed"] = bool(obj.Geometry.IsClosed)
                    definition["periodic"] = bool(obj.Geometry.IsPeriodic)
                    definitions.append(definition)
                return {"curves": definitions}
            finally:
                for obj in list(document.Objects):
                    if obj.Id not in before_all:
                        document.Objects.Delete(obj.Id, True)

        return _measure(iterations, create_curves_through_geometry)

    if kind == "curve_tween_geometry":
        curves = []
        try:
            for curve_name in ("start_curve", "end_curve"):
                curves.append(_nurbs_curve_from_definition(operation[curve_name]))

            method = operation["method"]
            num_curves = int(operation["number"])
            num_samples = int(operation.get("sample_number", 100))

            def create_tween_curves():
                if method == "control_point":
                    result = Rhino.Geometry.Curve.CreateTweenCurves(
                        curves[0], curves[1], num_curves, tolerance["absolute"]
                    )
                elif method == "refit":
                    result = Rhino.Geometry.Curve.CreateTweenCurvesWithMatching(
                        curves[0], curves[1], num_curves, tolerance["absolute"]
                    )
                elif method == "sample_points":
                    result = Rhino.Geometry.Curve.CreateTweenCurvesWithSampling(
                        curves[0],
                        curves[1],
                        num_curves,
                        num_samples,
                        tolerance["absolute"],
                    )
                else:
                    raise ValueError("unknown curve tween method")
                if result is None:
                    raise ValueError("Rhino curve tween returned no result")
                try:
                    return {
                        "curves": [_nurbs_curve_definition(curve) for curve in result]
                    }
                finally:
                    for curve in result:
                        curve.Dispose()

            return _measure(iterations, create_tween_curves)
        finally:
            for curve in curves:
                curve.Dispose()

    if kind == "curve_fit_geometry":
        curve = _nurbs_curve_from_definition(operation["curve"])
        degree = int(operation["degree"])
        fit_tolerance = _finite(operation["fit_tolerance"], "curve fit tolerance")
        angle_tolerance = _finite(
            operation.get("angle_tolerance_radians", tolerance["angular"]),
            "curve fit angle tolerance",
        )

        def fit_curve():
            result = curve.Fit(degree, fit_tolerance, angle_tolerance)
            if result is None:
                raise ValueError("Rhino curve fit returned no result")
            try:
                return _nurbs_curve_definition(result)
            finally:
                result.Dispose()

        try:
            return _measure(iterations, fit_curve)
        finally:
            curve.Dispose()

    if kind == "curve_rebuild_geometry":
        curve = _nurbs_curve_from_definition(operation["curve"])
        degree = int(operation["degree"])
        point_count = int(operation["point_count"])
        preserve_tangents = bool(operation.get("preserve_tangents", False))

        def rebuild_curve():
            result = curve.Rebuild(point_count, degree, preserve_tangents)
            if result is None:
                raise ValueError("Rhino curve rebuild returned no result")
            try:
                definition = _nurbs_curve_definition(result)
                definition["closed"] = bool(result.IsClosed)
                definition["periodic"] = bool(result.IsPeriodic)
                return definition
            finally:
                result.Dispose()

        try:
            return _measure(iterations, rebuild_curve)
        finally:
            curve.Dispose()

    if kind == "curve_make_uniform_geometry":
        curve = _nurbs_curve_from_definition(operation["curve"])

        def make_uniform_curve():
            duplicate = curve.DuplicateCurve()
            if duplicate is None:
                raise ValueError("Rhino could not duplicate curve for uniformization")
            try:
                nurbs = duplicate.ToNurbsCurve()
                if nurbs is None:
                    raise ValueError("Rhino could not convert curve for uniformization")
                try:
                    if not nurbs.MakeUniform():
                        raise ValueError("Rhino curve uniformization failed")
                    definition = _nurbs_curve_definition(nurbs)
                    definition["closed"] = bool(nurbs.IsClosed)
                    definition["periodic"] = bool(nurbs.IsPeriodic)
                    return definition
                finally:
                    nurbs.Dispose()
            finally:
                duplicate.Dispose()

        try:
            return _measure(iterations, make_uniform_curve)
        finally:
            curve.Dispose()

    if kind == "curve_insert_knot_geometry":
        curve = _nurbs_curve_from_definition(operation["curve"])
        parameter = _finite(operation["parameter"], "curve knot parameter")
        multiplicity = int(operation["multiplicity"])

        def insert_curve_knot():
            duplicate = curve.DuplicateCurve()
            if duplicate is None:
                raise ValueError("Rhino could not duplicate curve for knot insertion")
            try:
                nurbs = duplicate.ToNurbsCurve()
                if nurbs is None:
                    raise ValueError("Rhino could not convert curve for knot insertion")
                try:
                    if not nurbs.Knots.InsertKnot(parameter, multiplicity):
                        raise ValueError("Rhino curve knot insertion failed")
                    definition = _nurbs_curve_definition(nurbs)
                    definition["closed"] = bool(nurbs.IsClosed)
                    definition["periodic"] = bool(nurbs.IsPeriodic)
                    return definition
                finally:
                    nurbs.Dispose()
            finally:
                duplicate.Dispose()

        try:
            return _measure(iterations, insert_curve_knot)
        finally:
            curve.Dispose()

    if kind == "curve_remove_knot_geometry":
        curve = _nurbs_curve_from_definition(operation["curve"])
        parameter = _finite(operation["parameter"], "curve knot parameter")

        def remove_curve_knot():
            nurbs = curve.Duplicate()
            if nurbs is None:
                raise ValueError("Rhino could not duplicate curve for knot removal")
            try:
                if not isinstance(nurbs, Rhino.Geometry.NurbsCurve):
                    raise ValueError("Rhino duplicate was not a NURBS curve")
                if not nurbs.Knots.RemoveKnotAt(parameter):
                    raise ValueError("Rhino curve knot removal failed")
                definition = _nurbs_curve_definition(nurbs)
                definition["closed"] = bool(nurbs.IsClosed)
                definition["periodic"] = bool(nurbs.IsPeriodic)
                return definition
            finally:
                nurbs.Dispose()

        try:
            return _measure(iterations, remove_curve_knot)
        finally:
            curve.Dispose()

    if kind == "curve_remove_multi_knot_geometry":
        document = Rhino.RhinoDoc.ActiveDoc
        source = _nurbs_curve_from_definition(operation["curve"])
        remove_fully = bool(operation.get("remove_fully_multiple_knots", False))
        max_kink_angle = _finite(
            operation.get("maximum_kink_angle_degrees", 1.0),
            "maximum kink angle",
        )

        def remove_multi_knot_command():
            object_id = System.Guid.Empty
            try:
                document.Objects.UnselectAll()
                object_id = document.Objects.AddCurve(source)
                if object_id == System.Guid.Empty:
                    raise ValueError("could not add RemoveMultiKnot source curve")
                command = "_-RemoveMultiKnot _RemoveFullyMultipleKnots=_%s" % (
                    "Yes" if remove_fully else "No"
                )
                if remove_fully:
                    command += " _MaxKinkAngle=%.17g" % max_kink_angle
                command += " _SelID %s _Enter" % str(object_id)
                Rhino.RhinoApp.RunScript(command, False)
                rhino_object = document.Objects.FindId(object_id)
                if rhino_object is None:
                    raise ValueError("RemoveMultiKnot removed the source object")
                result = rhino_object.Geometry.ToNurbsCurve()
                if result is None:
                    raise ValueError("RemoveMultiKnot returned no NURBS curve")
                try:
                    definition = _nurbs_curve_definition(result)
                    definition["closed"] = bool(result.IsClosed)
                    definition["periodic"] = bool(result.IsPeriodic)
                    return definition
                finally:
                    result.Dispose()
            finally:
                if object_id != System.Guid.Empty:
                    document.Objects.Delete(object_id, True)

        try:
            return _measure(iterations, remove_multi_knot_command)
        finally:
            source.Dispose()

    if kind == "curve_change_seam_geometry":
        source = _nurbs_curve_from_definition(operation["curve"])
        parameter = _finite(operation["parameter"], "closed curve seam parameter")

        def change_curve_seam():
            duplicate = source.DuplicateCurve()
            if duplicate is None:
                raise ValueError("Rhino could not duplicate curve for seam relocation")
            try:
                if not duplicate.ChangeClosedCurveSeam(parameter):
                    raise ValueError("Rhino closed curve seam relocation failed")
                nurbs = duplicate.ToNurbsCurve()
                if nurbs is None:
                    raise ValueError("Rhino seam relocation returned no NURBS curve")
                try:
                    definition = _nurbs_curve_definition(nurbs)
                    definition["closed"] = bool(nurbs.IsClosed)
                    definition["periodic"] = bool(nurbs.IsPeriodic)
                    return definition
                finally:
                    nurbs.Dispose()
            finally:
                duplicate.Dispose()

        try:
            return _measure(iterations, change_curve_seam)
        finally:
            source.Dispose()

    if kind == "curve_reparameterize_geometry":
        source = _nurbs_curve_from_definition(operation["curve"])
        try:
            domain = operation.get("domain")
            if domain is None:
                domain = [0.0, float(source.GetLength())]
            if len(domain) != 2:
                raise ValueError("curve reparameterization requires a two-value domain")
            target = Rhino.Geometry.Interval(
                _finite(domain[0], "curve domain start"),
                _finite(domain[1], "curve domain end"),
            )
            if not target.IsIncreasing:
                raise ValueError("curve reparameterization domain must be increasing")
        except Exception:
            source.Dispose()
            raise

        def reparameterize_curve():
            duplicate = source.DuplicateCurve()
            if duplicate is None:
                raise ValueError("Rhino could not duplicate curve for reparameterization")
            try:
                duplicate.Domain = target
                actual = duplicate.Domain
                if actual.T0 != target.T0 or actual.T1 != target.T1:
                    raise ValueError("Rhino curve reparameterization failed")
                definition = _nurbs_curve_definition(duplicate)
                definition["closed"] = bool(duplicate.IsClosed)
                definition["periodic"] = bool(duplicate.IsPeriodic)
                return definition
            finally:
                duplicate.Dispose()

        try:
            return _measure(iterations, reparameterize_curve)
        finally:
            source.Dispose()

    if kind == "curve_extend_geometry":
        source = _nurbs_curve_from_definition(operation["curve"])
        values = operation["domain"]
        if len(values) != 2:
            source.Dispose()
            raise ValueError("curve extension domain requires two parameters")
        target = Rhino.Geometry.Interval(
            _finite(values[0], "curve extension domain start"),
            _finite(values[1], "curve extension domain end"),
        )
        if not target.IsIncreasing:
            source.Dispose()
            raise ValueError("curve extension domain must be increasing")

        def extend_curve():
            result = source.Extend(target)
            if result is None:
                raise ValueError("Rhino natural curve extension failed")
            try:
                definition = _nurbs_curve_definition(result)
                definition["closed"] = bool(result.IsClosed)
                definition["periodic"] = bool(result.IsPeriodic)
                return definition
            finally:
                result.Dispose()

        try:
            return _measure(iterations, extend_curve)
        finally:
            source.Dispose()

    if kind == "curve_extend_length_geometry":
        source = _nurbs_curve_from_definition(operation["curve"])
        length = _finite(operation["length"], "curve extension length")
        if not length > 0.0:
            source.Dispose()
            raise ValueError("curve extension length must be positive")
        side_name = str(operation["side"]).lower()
        sides = {
            "start": Rhino.Geometry.CurveEnd.Start,
            "end": Rhino.Geometry.CurveEnd.End,
            "both": Rhino.Geometry.CurveEnd.Both,
        }
        style_name = str(operation.get("style", "smooth")).lower()
        styles = {
            "line": Rhino.Geometry.CurveExtensionStyle.Line,
            "arc": Rhino.Geometry.CurveExtensionStyle.Arc,
            "smooth": Rhino.Geometry.CurveExtensionStyle.Smooth,
        }
        if side_name not in sides or style_name not in styles:
            source.Dispose()
            raise ValueError("invalid curve extension side or style")

        def extend_curve_by_length():
            result = source.Extend(sides[side_name], length, styles[style_name])
            if result is None:
                raise ValueError("Rhino curve length extension failed")
            try:
                definition = _nurbs_curve_definition(result)
                definition["closed"] = bool(result.IsClosed)
                definition["periodic"] = bool(result.IsPeriodic)
                return definition
            finally:
                result.Dispose()

        try:
            return _measure(iterations, extend_curve_by_length)
        finally:
            source.Dispose()

    if kind == "curve_extend_command":
        document = Rhino.RhinoDoc.ActiveDoc
        source = _nurbs_curve_from_definition(operation["curve"])
        length = _finite(operation["length"], "curve extension length")
        if not length > 0.0:
            source.Dispose()
            raise ValueError("curve extension length must be positive")
        side_name = str(operation["side"]).lower()
        style_name = str(operation["style"]).lower()
        join_name = str(operation["join"]).lower()
        if side_name not in ("start", "end", "both"):
            source.Dispose()
            raise ValueError("invalid curve extension side")
        if style_name not in ("natural", "arc", "line", "smooth"):
            source.Dispose()
            raise ValueError("invalid curve extension style")
        if join_name not in ("merge", "yes", "no"):
            source.Dispose()
            raise ValueError("invalid curve extension join mode")

        def crossing_selection(point, radius):
            return "_SelCrossing %.17g,%.17g %.17g,%.17g" % (
                point.X - radius,
                point.Y - radius,
                point.X + radius,
                point.Y + radius,
            )

        def extend_curve_command():
            original_ids = set(item.Id for item in document.Objects)
            object_id = System.Guid.Empty
            group_index = -1
            try:
                document.Objects.UnselectAll()
                attributes = Rhino.DocObjects.ObjectAttributes()
                attributes.Name = "Viboceros Extend Source"
                attributes.ObjectColor = System.Drawing.Color.FromArgb(12, 34, 56)
                attributes.ColorSource = Rhino.DocObjects.ObjectColorSource.ColorFromObject
                object_id = document.Objects.AddCurve(source, attributes)
                if object_id == System.Guid.Empty:
                    raise ValueError("could not add Extend command source curve")
                group_index = document.Groups.Add(
                    "Viboceros Extend Group " + str(System.Guid.NewGuid()),
                    [object_id],
                )
                if group_index < 0:
                    raise ValueError("could not group Extend command source curve")
                Rhino.RhinoApp.RunScript("_-SetView _World _Top _Zoom _Extents", False)
                radius = max(1.0, float(source.GetBoundingBox(True).Diagonal.Length)) * 0.02
                selections = []
                if side_name in ("start", "both"):
                    selections.append(crossing_selection(source.PointAtStart, radius))
                if side_name in ("end", "both"):
                    selections.append(crossing_selection(source.PointAtEnd, radius))
                command = "_-Extend _Type=_%s _Join=_%s %.17g %s _Enter" % (
                    (
                        "Natural"
                        if style_name == "natural"
                        else (
                            "Arc"
                            if style_name == "arc"
                            else ("Line" if style_name == "line" else "Smooth")
                        )
                    ),
                    (
                        "Merge"
                        if join_name == "merge"
                        else ("Yes" if join_name == "yes" else "No")
                    ),
                    length,
                    " ".join(selections),
                )
                succeeded = Rhino.RhinoApp.RunScript(command, False)
                objects = [
                    item
                    for item in document.Objects
                    if item.Id not in original_ids
                    and isinstance(item.Geometry, Rhino.Geometry.Curve)
                ]
                expected_count = 1 + (
                    (2 if side_name == "both" else 1) if join_name == "no" else 0
                )
                if len(objects) != expected_count:
                    history = Rhino.RhinoApp.CommandHistoryWindowText
                    raise ValueError(
                        "Extend macro %r returned %r and left %d curve objects; "
                        "expected %d; history tail: %s"
                        % (
                            command,
                            succeeded,
                            len(objects),
                            expected_count,
                            history[-3000:],
                        )
                    )
                records = []
                for item in objects:
                    groups = item.Attributes.GetGroupList()
                    color = item.Attributes.ObjectColor
                    records.append({
                        "attributes_match_source": (
                            item.Attributes.Name == "Viboceros Extend Source"
                            and int(item.Attributes.LayerIndex) == int(attributes.LayerIndex)
                            and int(color.R) == 12
                            and int(color.G) == 34
                            and int(color.B) == 56
                            and item.Attributes.ColorSource
                            == Rhino.DocObjects.ObjectColorSource.ColorFromObject
                        ),
                        "curve": _nurbs_curve_definition(item.Geometry),
                        "in_source_group": (
                            groups is not None and group_index in groups
                        ),
                        "original_id": item.Id == object_id,
                        "selected": item.IsSelected(False) > 0,
                    })
                records.sort(
                    key=lambda record: (
                        record["original_id"],
                        tuple(record["curve"]["control_points"][0]["point"]),
                    )
                )
                return {
                    "command_succeeded": bool(succeeded),
                    "objects": records,
                }
            finally:
                document.Objects.UnselectAll()
                for item in list(document.Objects):
                    if item.Id not in original_ids:
                        document.Objects.Delete(item.Id, True)
                if group_index >= 0 and not document.Groups.IsDeleted(group_index):
                    document.Groups.Delete(group_index)

        try:
            return _measure(iterations, extend_curve_command)
        finally:
            source.Dispose()

    if kind == "curve_extend_boundary_command":
        document = Rhino.RhinoDoc.ActiveDoc
        source = _nurbs_curve_from_definition(operation["curve"])
        boundaries = []
        try:
            for definition in operation["boundaries"]:
                boundaries.append(
                    _curve_extension_boundary_from_definition(definition, tolerance)
                )
        except Exception:
            source.Dispose()
            for boundary in boundaries:
                boundary.Dispose()
            raise
        side_name = str(operation["side"]).lower()
        style_name = str(operation["style"]).lower()
        join_name = str(operation["join"]).lower()
        if not boundaries or side_name not in ("start", "end", "both"):
            source.Dispose()
            for boundary in boundaries:
                boundary.Dispose()
            raise ValueError("invalid curve boundary extension side")
        if style_name not in ("natural", "arc", "line", "smooth"):
            source.Dispose()
            for boundary in boundaries:
                boundary.Dispose()
            raise ValueError("invalid curve boundary extension style")
        if join_name not in ("merge", "yes", "no"):
            source.Dispose()
            for boundary in boundaries:
                boundary.Dispose()
            raise ValueError("invalid curve boundary extension join mode")

        def crossing_selection(point, radius):
            return "_SelCrossing %.17g,%.17g %.17g,%.17g" % (
                point.X - radius,
                point.Y - radius,
                point.X + radius,
                point.Y + radius,
            )

        def extend_curve_to_boundaries_command():
            original_ids = set(item.Id for item in document.Objects)
            source_id = System.Guid.Empty
            boundary_ids = []
            group_index = -1
            try:
                document.Objects.UnselectAll()
                attributes = Rhino.DocObjects.ObjectAttributes()
                attributes.Name = "Viboceros Extend Source"
                attributes.ObjectColor = System.Drawing.Color.FromArgb(12, 34, 56)
                attributes.ColorSource = Rhino.DocObjects.ObjectColorSource.ColorFromObject
                source_id = document.Objects.AddCurve(source, attributes)
                if source_id == System.Guid.Empty:
                    raise ValueError("could not add Extend command source curve")
                group_index = document.Groups.Add(
                    "Viboceros Extend Group " + str(System.Guid.NewGuid()),
                    [source_id],
                )
                if group_index < 0:
                    raise ValueError("could not group Extend command source curve")
                for boundary in boundaries:
                    if isinstance(boundary, Rhino.Geometry.Curve):
                        boundary_id = document.Objects.AddCurve(boundary)
                    elif isinstance(boundary, Rhino.Geometry.Surface):
                        boundary_id = document.Objects.AddSurface(boundary)
                    elif isinstance(boundary, Rhino.Geometry.Brep):
                        boundary_id = document.Objects.AddBrep(boundary)
                    else:
                        raise ValueError("unsupported Extend boundary geometry")
                    if boundary_id == System.Guid.Empty:
                        raise ValueError("could not add Extend command boundary curve")
                    boundary_ids.append(boundary_id)
                    document.Objects.Select(boundary_id)
                Rhino.RhinoApp.RunScript("_-SetView _World _Top _Zoom _Extents", False)
                radius = max(1.0, float(source.GetBoundingBox(True).Diagonal.Length)) * 0.02
                selections = []
                if side_name in ("start", "both"):
                    selections.append(crossing_selection(source.PointAtStart, radius))
                if side_name in ("end", "both"):
                    selections.append(crossing_selection(source.PointAtEnd, radius))
                command = "_-Extend _Type=_%s _Join=_%s %s _Enter" % (
                    (
                        "Natural"
                        if style_name == "natural"
                        else (
                            "Arc"
                            if style_name == "arc"
                            else ("Line" if style_name == "line" else "Smooth")
                        )
                    ),
                    (
                        "Merge"
                        if join_name == "merge"
                        else ("Yes" if join_name == "yes" else "No")
                    ),
                    " ".join(selections),
                )
                succeeded = Rhino.RhinoApp.RunScript(command, False)
                objects = [
                    item
                    for item in document.Objects
                    if item.Id not in original_ids
                    and item.Id not in boundary_ids
                    and isinstance(item.Geometry, Rhino.Geometry.Curve)
                ]
                expected_count = 1 + (
                    (2 if side_name == "both" else 1) if join_name == "no" else 0
                )
                if len(objects) != expected_count:
                    history = Rhino.RhinoApp.CommandHistoryWindowText
                    raise ValueError(
                        "boundary Extend macro %r returned %r and left %d result curves; "
                        "expected %d; history tail: %s"
                        % (
                            command,
                            succeeded,
                            len(objects),
                            expected_count,
                            history[-3000:],
                        )
                    )
                records = []
                for item in objects:
                    groups = item.Attributes.GetGroupList()
                    color = item.Attributes.ObjectColor
                    records.append({
                        "attributes_match_source": (
                            item.Attributes.Name == "Viboceros Extend Source"
                            and int(item.Attributes.LayerIndex) == int(attributes.LayerIndex)
                            and int(color.R) == 12
                            and int(color.G) == 34
                            and int(color.B) == 56
                            and item.Attributes.ColorSource
                            == Rhino.DocObjects.ObjectColorSource.ColorFromObject
                        ),
                        "curve": _nurbs_curve_definition(
                            item.Geometry,
                            bool(operation.get("canonicalize_curve_parameters", False)),
                        ),
                        "in_source_group": (
                            groups is not None and group_index in groups
                        ),
                        "original_id": item.Id == source_id,
                        "selected": item.IsSelected(False) > 0,
                    })
                records.sort(
                    key=lambda record: (
                        record["original_id"],
                        tuple(record["curve"]["control_points"][0]["point"]),
                    )
                )
                return {
                    "command_succeeded": bool(succeeded),
                    "objects": records,
                }
            finally:
                document.Objects.UnselectAll()
                for item in list(document.Objects):
                    if item.Id not in original_ids:
                        document.Objects.Delete(item.Id, True)
                if group_index >= 0 and not document.Groups.IsDeleted(group_index):
                    document.Groups.Delete(group_index)

        try:
            return _measure(iterations, extend_curve_to_boundaries_command)
        finally:
            source.Dispose()
            for boundary in boundaries:
                boundary.Dispose()

    if kind == "curve_subcurve_geometry":
        source = _nurbs_curve_from_definition(operation["curve"])
        start = _finite(operation["start"], "subcurve start parameter")
        end = _finite(operation["end"], "subcurve end parameter")

        def extract_subcurve():
            if source.IsClosed or start < end:
                result = source.Trim(Rhino.Geometry.Interval(start, end))
            else:
                result = source.Trim(Rhino.Geometry.Interval(end, start))
                if result is not None and not result.Reverse():
                    result.Dispose()
                    raise ValueError("Rhino could not reverse the open subcurve")
            if result is None:
                raise ValueError("Rhino subcurve extraction failed")
            try:
                definition = _nurbs_curve_definition(result)
                definition["closed"] = bool(result.IsClosed)
                definition["periodic"] = bool(result.IsPeriodic)
                return definition
            finally:
                result.Dispose()

        try:
            return _measure(iterations, extract_subcurve)
        finally:
            source.Dispose()

    if kind == "curve_intersect_command":
        document = Rhino.RhinoDoc.ActiveDoc
        curves = [
            _nurbs_curve_from_definition(definition)
            for definition in operation["curves"]
        ]

        def intersect_curves_command():
            original_ids = set(item.Id for item in document.Objects)
            input_ids = []
            group_index = -1
            try:
                document.Objects.UnselectAll()
                attributes = Rhino.DocObjects.ObjectAttributes()
                attributes.Name = "Viboceros Intersect Source"
                attributes.ObjectColor = System.Drawing.Color.FromArgb(12, 34, 56)
                attributes.ColorSource = Rhino.DocObjects.ObjectColorSource.ColorFromObject
                for curve in curves:
                    object_id = document.Objects.AddCurve(curve, attributes)
                    if object_id == System.Guid.Empty:
                        raise ValueError("could not add Intersect command input curve")
                    input_ids.append(object_id)
                    document.Objects.Select(object_id)
                group_index = document.Groups.Add(
                    "Viboceros Intersect Group " + str(System.Guid.NewGuid()),
                    input_ids,
                )
                if group_index < 0:
                    raise ValueError("could not group Intersect command input curves")
                succeeded = Rhino.RhinoApp.RunScript("_-Intersect _Enter", False)
                records = []
                for item in document.Objects:
                    if item.Id in original_ids or item.Id in input_ids:
                        continue
                    geometry = item.Geometry
                    if isinstance(geometry, Rhino.Geometry.Point):
                        location = geometry.Location
                        value = {
                            "kind": "point",
                            "point": [
                                float(location.X),
                                float(location.Y),
                                float(location.Z),
                            ],
                        }
                        sort_key = ("point", float(location.X), float(location.Y), float(location.Z))
                    elif isinstance(geometry, Rhino.Geometry.Curve):
                        definition = _nurbs_curve_definition(geometry)
                        value = {"kind": "curve", "curve": definition}
                        sort_key = ("curve",) + tuple(
                            definition["control_points"][0]["point"]
                        )
                    else:
                        raise ValueError(
                            "Intersect produced unsupported geometry %s"
                            % type(geometry).__name__
                        )
                    groups = item.Attributes.GetGroupList()
                    value.update({
                        "blank_name": not bool(item.Attributes.Name),
                        "color_from_layer": (
                            item.Attributes.ColorSource
                            == Rhino.DocObjects.ObjectColorSource.ColorFromLayer
                        ),
                        "in_source_group": (
                            groups is not None and group_index in groups
                        ),
                        "on_current_layer": (
                            int(item.Attributes.LayerIndex)
                            == int(document.Layers.CurrentLayerIndex)
                        ),
                        "selected": item.IsSelected(False) > 0,
                    })
                    records.append((sort_key, value))
                records.sort(key=lambda record: record[0])
                return {
                    "command_succeeded": bool(succeeded),
                    "input_selected": [
                        document.Objects.FindId(object_id).IsSelected(False) > 0
                        for object_id in input_ids
                    ],
                    "objects": [value for _, value in records],
                }
            finally:
                document.Objects.UnselectAll()
                for item in list(document.Objects):
                    if item.Id not in original_ids:
                        document.Objects.Delete(item.Id, True)
                if group_index >= 0 and not document.Groups.IsDeleted(group_index):
                    document.Groups.Delete(group_index)

        try:
            return _measure(iterations, intersect_curves_command)
        finally:
            for curve in curves:
                curve.Dispose()

    if kind == "curve_surface_intersect_command":
        document = Rhino.RhinoDoc.ActiveDoc
        curve = _nurbs_curve_from_definition(operation["curve"])
        surface = _nurbs_surface_from_definition(operation["surface"])

        def intersect_curve_surface_command():
            original_ids = set(item.Id for item in document.Objects)
            input_ids = []
            group_index = -1
            try:
                document.Objects.UnselectAll()
                attributes = Rhino.DocObjects.ObjectAttributes()
                attributes.Name = "Viboceros Intersect Source"
                attributes.ObjectColor = System.Drawing.Color.FromArgb(12, 34, 56)
                attributes.ColorSource = Rhino.DocObjects.ObjectColorSource.ColorFromObject
                curve_id = document.Objects.AddCurve(curve, attributes)
                surface_id = document.Objects.AddSurface(surface, attributes)
                if curve_id == System.Guid.Empty or surface_id == System.Guid.Empty:
                    raise ValueError("could not add curve/surface Intersect inputs")
                input_ids.extend([curve_id, surface_id])
                for object_id in input_ids:
                    document.Objects.Select(object_id)
                group_index = document.Groups.Add(
                    "Viboceros Intersect Group " + str(System.Guid.NewGuid()),
                    input_ids,
                )
                if group_index < 0:
                    raise ValueError("could not group curve/surface Intersect inputs")
                succeeded = Rhino.RhinoApp.RunScript("_-Intersect _Enter", False)
                records = []
                for item in document.Objects:
                    if item.Id in original_ids or item.Id in input_ids:
                        continue
                    geometry = item.Geometry
                    if isinstance(geometry, Rhino.Geometry.Point):
                        location = geometry.Location
                        value = {
                            "kind": "point",
                            "point": [
                                float(location.X),
                                float(location.Y),
                                float(location.Z),
                            ],
                        }
                        sort_key = (
                            "point",
                            float(location.X),
                            float(location.Y),
                            float(location.Z),
                        )
                    elif isinstance(geometry, Rhino.Geometry.Curve):
                        definition = _nurbs_curve_definition(geometry)
                        value = {"kind": "curve", "curve": definition}
                        sort_key = ("curve",) + tuple(
                            definition["control_points"][0]["point"]
                        )
                    else:
                        raise ValueError(
                            "curve/surface Intersect produced unsupported geometry %s"
                            % type(geometry).__name__
                        )
                    groups = item.Attributes.GetGroupList()
                    value.update({
                        "blank_name": not bool(item.Attributes.Name),
                        "color_from_layer": (
                            item.Attributes.ColorSource
                            == Rhino.DocObjects.ObjectColorSource.ColorFromLayer
                        ),
                        "in_source_group": (
                            groups is not None and group_index in groups
                        ),
                        "on_current_layer": (
                            int(item.Attributes.LayerIndex)
                            == int(document.Layers.CurrentLayerIndex)
                        ),
                        "selected": item.IsSelected(False) > 0,
                    })
                    records.append((sort_key, value))
                records.sort(key=lambda record: record[0])
                return {
                    "command_succeeded": bool(succeeded),
                    "input_selected": [
                        document.Objects.FindId(object_id).IsSelected(False) > 0
                        for object_id in input_ids
                    ],
                    "objects": [value for _, value in records],
                }
            finally:
                document.Objects.UnselectAll()
                for item in list(document.Objects):
                    if item.Id not in original_ids:
                        document.Objects.Delete(item.Id, True)
                if group_index >= 0 and not document.Groups.IsDeleted(group_index):
                    document.Groups.Delete(group_index)

        try:
            return _measure(iterations, intersect_curve_surface_command)
        finally:
            curve.Dispose()
            surface.Dispose()

    if kind == "curve_brep_intersect_command":
        document = Rhino.RhinoDoc.ActiveDoc
        curve = _nurbs_curve_from_definition(operation["curve"])
        box_min = _point(operation["box_min"])
        box_max = _point(operation["box_max"])
        brep = Rhino.Geometry.Brep.CreateFromBox(
            Rhino.Geometry.BoundingBox(box_min, box_max)
        )
        if brep is None:
            curve.Dispose()
            raise ValueError("could not create curve/B-rep Intersect box")

        def intersect_curve_brep_command():
            original_ids = set(item.Id for item in document.Objects)
            input_ids = []
            group_index = -1
            try:
                document.Objects.UnselectAll()
                attributes = Rhino.DocObjects.ObjectAttributes()
                attributes.Name = "Viboceros Intersect Source"
                attributes.ObjectColor = System.Drawing.Color.FromArgb(12, 34, 56)
                attributes.ColorSource = Rhino.DocObjects.ObjectColorSource.ColorFromObject
                curve_id = document.Objects.AddCurve(curve, attributes)
                brep_id = document.Objects.AddBrep(brep, attributes)
                if curve_id == System.Guid.Empty or brep_id == System.Guid.Empty:
                    raise ValueError("could not add curve/B-rep Intersect inputs")
                input_ids.extend([curve_id, brep_id])
                for object_id in input_ids:
                    document.Objects.Select(object_id)
                group_index = document.Groups.Add(
                    "Viboceros Intersect Group " + str(System.Guid.NewGuid()),
                    input_ids,
                )
                if group_index < 0:
                    raise ValueError("could not group curve/B-rep Intersect inputs")
                succeeded = Rhino.RhinoApp.RunScript("_-Intersect _Enter", False)
                records = []
                for item in document.Objects:
                    if item.Id in original_ids or item.Id in input_ids:
                        continue
                    geometry = item.Geometry
                    if isinstance(geometry, Rhino.Geometry.Point):
                        location = geometry.Location
                        value = {
                            "kind": "point",
                            "point": [
                                float(location.X),
                                float(location.Y),
                                float(location.Z),
                            ],
                        }
                        sort_key = (
                            "point",
                            float(location.X),
                            float(location.Y),
                            float(location.Z),
                        )
                    elif isinstance(geometry, Rhino.Geometry.Curve):
                        definition = _nurbs_curve_definition(geometry)
                        value = {"kind": "curve", "curve": definition}
                        sort_key = ("curve",) + tuple(
                            definition["control_points"][0]["point"]
                        )
                    else:
                        raise ValueError(
                            "curve/B-rep Intersect produced unsupported geometry %s"
                            % type(geometry).__name__
                        )
                    groups = item.Attributes.GetGroupList()
                    value.update({
                        "blank_name": not bool(item.Attributes.Name),
                        "color_from_layer": (
                            item.Attributes.ColorSource
                            == Rhino.DocObjects.ObjectColorSource.ColorFromLayer
                        ),
                        "in_source_group": (
                            groups is not None and group_index in groups
                        ),
                        "on_current_layer": (
                            int(item.Attributes.LayerIndex)
                            == int(document.Layers.CurrentLayerIndex)
                        ),
                        "selected": item.IsSelected(False) > 0,
                    })
                    records.append((sort_key, value))
                records.sort(key=lambda record: record[0])
                return {
                    "command_succeeded": bool(succeeded),
                    "input_selected": [
                        document.Objects.FindId(object_id).IsSelected(False) > 0
                        for object_id in input_ids
                    ],
                    "objects": [value for _, value in records],
                }
            finally:
                document.Objects.UnselectAll()
                for item in list(document.Objects):
                    if item.Id not in original_ids:
                        document.Objects.Delete(item.Id, True)
                if group_index >= 0 and not document.Groups.IsDeleted(group_index):
                    document.Groups.Delete(group_index)

        try:
            return _measure(iterations, intersect_curve_brep_command)
        finally:
            curve.Dispose()
            brep.Dispose()

    if kind == "surface_surface_intersect_command":
        document = Rhino.RhinoDoc.ActiveDoc
        surfaces = [
            _nurbs_surface_from_definition(operation["first"]),
            _nurbs_surface_from_definition(operation["second"]),
        ]

        def intersect_surfaces_command():
            original_ids = set(item.Id for item in document.Objects)
            input_ids = []
            group_index = -1
            try:
                document.Objects.UnselectAll()
                attributes = Rhino.DocObjects.ObjectAttributes()
                attributes.Name = "Viboceros Intersect Source"
                attributes.ObjectColor = System.Drawing.Color.FromArgb(12, 34, 56)
                attributes.ColorSource = Rhino.DocObjects.ObjectColorSource.ColorFromObject
                for surface in surfaces:
                    object_id = document.Objects.AddSurface(surface, attributes)
                    if object_id == System.Guid.Empty:
                        raise ValueError("could not add surface/surface Intersect input")
                    input_ids.append(object_id)
                    document.Objects.Select(object_id)
                group_index = document.Groups.Add(
                    "Viboceros Intersect Group " + str(System.Guid.NewGuid()),
                    input_ids,
                )
                if group_index < 0:
                    raise ValueError("could not group surface/surface Intersect inputs")
                succeeded = Rhino.RhinoApp.RunScript("_-Intersect _Enter", False)
                records = []
                for item in document.Objects:
                    if item.Id in original_ids or item.Id in input_ids:
                        continue
                    geometry = item.Geometry
                    if isinstance(geometry, Rhino.Geometry.Point):
                        location = geometry.Location
                        value = {
                            "kind": "point",
                            "point": [
                                float(location.X),
                                float(location.Y),
                                float(location.Z),
                            ],
                        }
                        sort_key = (
                            "point",
                            float(location.X),
                            float(location.Y),
                            float(location.Z),
                        )
                    elif isinstance(geometry, Rhino.Geometry.Curve):
                        if operation.get("canonicalize_closed_curves", False):
                            definition = (
                                _canonical_closed_intersection_curve_definition(geometry)
                            )
                        else:
                            definition = _nurbs_curve_definition(geometry)
                        value = {"kind": "curve", "curve": definition}
                        sort_key = ("curve",) + tuple(
                            definition["control_points"][0]["point"]
                        )
                    else:
                        raise ValueError(
                            "surface/surface Intersect produced unsupported geometry %s"
                            % type(geometry).__name__
                        )
                    groups = item.Attributes.GetGroupList()
                    value.update({
                        "blank_name": not bool(item.Attributes.Name),
                        "color_from_layer": (
                            item.Attributes.ColorSource
                            == Rhino.DocObjects.ObjectColorSource.ColorFromLayer
                        ),
                        "in_source_group": (
                            groups is not None and group_index in groups
                        ),
                        "on_current_layer": (
                            int(item.Attributes.LayerIndex)
                            == int(document.Layers.CurrentLayerIndex)
                        ),
                        "selected": item.IsSelected(False) > 0,
                    })
                    records.append((sort_key, value))
                records.sort(key=lambda record: record[0])
                return {
                    "command_succeeded": bool(succeeded),
                    "input_selected": [
                        document.Objects.FindId(object_id).IsSelected(False) > 0
                        for object_id in input_ids
                    ],
                    "objects": [value for _, value in records],
                }
            finally:
                document.Objects.UnselectAll()
                for item in list(document.Objects):
                    if item.Id not in original_ids:
                        document.Objects.Delete(item.Id, True)
                if group_index >= 0 and not document.Groups.IsDeleted(group_index):
                    document.Groups.Delete(group_index)

        try:
            return _measure(iterations, intersect_surfaces_command)
        finally:
            for surface in surfaces:
                surface.Dispose()

    if kind == "surface_brep_intersect_command":
        document = Rhino.RhinoDoc.ActiveDoc
        surface = _nurbs_surface_from_definition(operation["surface"])
        box_min = _point(operation["box_min"])
        box_max = _point(operation["box_max"])
        brep = Rhino.Geometry.Brep.CreateFromBox(
            Rhino.Geometry.BoundingBox(box_min, box_max)
        )
        if brep is None:
            surface.Dispose()
            raise ValueError("could not create surface/B-rep Intersect box")

        def intersect_surface_brep_command():
            original_ids = set(item.Id for item in document.Objects)
            input_ids = []
            group_index = -1
            try:
                document.Objects.UnselectAll()
                attributes = Rhino.DocObjects.ObjectAttributes()
                attributes.Name = "Viboceros Intersect Source"
                attributes.ObjectColor = System.Drawing.Color.FromArgb(12, 34, 56)
                attributes.ColorSource = Rhino.DocObjects.ObjectColorSource.ColorFromObject
                surface_id = document.Objects.AddSurface(surface, attributes)
                brep_id = document.Objects.AddBrep(brep, attributes)
                if surface_id == System.Guid.Empty or brep_id == System.Guid.Empty:
                    raise ValueError("could not add surface/B-rep Intersect inputs")
                if operation.get("brep_first", False):
                    input_ids.extend([brep_id, surface_id])
                else:
                    input_ids.extend([surface_id, brep_id])
                for object_id in input_ids:
                    document.Objects.Select(object_id)
                group_index = document.Groups.Add(
                    "Viboceros Intersect Group " + str(System.Guid.NewGuid()),
                    input_ids,
                )
                if group_index < 0:
                    raise ValueError("could not group surface/B-rep Intersect inputs")
                succeeded = Rhino.RhinoApp.RunScript("_-Intersect _Enter", False)
                records = []
                for item in document.Objects:
                    if item.Id in original_ids or item.Id in input_ids:
                        continue
                    geometry = item.Geometry
                    if isinstance(geometry, Rhino.Geometry.Point):
                        location = geometry.Location
                        value = {
                            "kind": "point",
                            "point": [
                                float(location.X),
                                float(location.Y),
                                float(location.Z),
                            ],
                        }
                        sort_key = (
                            "point",
                            float(location.X),
                            float(location.Y),
                            float(location.Z),
                        )
                    elif isinstance(geometry, Rhino.Geometry.Curve):
                        if operation.get("canonicalize_linear_curves", False):
                            definition = (
                                _canonical_linear_intersection_curve_definition(geometry)
                            )
                        else:
                            definition = _nurbs_curve_definition(geometry)
                        value = {"kind": "curve", "curve": definition}
                        sort_key = ("curve",) + tuple(
                            definition["control_points"][0]["point"]
                        )
                    else:
                        raise ValueError(
                            "surface/B-rep Intersect produced unsupported geometry %s"
                            % type(geometry).__name__
                        )
                    groups = item.Attributes.GetGroupList()
                    value.update({
                        "blank_name": not bool(item.Attributes.Name),
                        "color_from_layer": (
                            item.Attributes.ColorSource
                            == Rhino.DocObjects.ObjectColorSource.ColorFromLayer
                        ),
                        "in_source_group": (
                            groups is not None and group_index in groups
                        ),
                        "on_current_layer": (
                            int(item.Attributes.LayerIndex)
                            == int(document.Layers.CurrentLayerIndex)
                        ),
                        "selected": item.IsSelected(False) > 0,
                    })
                    records.append((sort_key, value))
                records.sort(key=lambda record: record[0])
                return {
                    "command_succeeded": bool(succeeded),
                    "input_selected": [
                        document.Objects.FindId(object_id).IsSelected(False) > 0
                        for object_id in input_ids
                    ],
                    "objects": [value for _, value in records],
                }
            finally:
                document.Objects.UnselectAll()
                for item in list(document.Objects):
                    if item.Id not in original_ids:
                        document.Objects.Delete(item.Id, True)
                if group_index >= 0 and not document.Groups.IsDeleted(group_index):
                    document.Groups.Delete(group_index)

        try:
            return _measure(iterations, intersect_surface_brep_command)
        finally:
            surface.Dispose()
            brep.Dispose()

    if kind == "brep_brep_intersect_command":
        document = Rhino.RhinoDoc.ActiveDoc
        first = Rhino.Geometry.Brep.CreateFromBox(
            Rhino.Geometry.BoundingBox(
                _point(operation["first_box_min"]),
                _point(operation["first_box_max"]),
            )
        )
        second = Rhino.Geometry.Brep.CreateFromBox(
            Rhino.Geometry.BoundingBox(
                _point(operation["second_box_min"]),
                _point(operation["second_box_max"]),
            )
        )
        if first is None or second is None:
            if first is not None:
                first.Dispose()
            if second is not None:
                second.Dispose()
            raise ValueError("could not create B-rep/B-rep Intersect boxes")

        def intersect_breps_command():
            original_ids = set(item.Id for item in document.Objects)
            input_ids = []
            group_index = -1
            try:
                document.Objects.UnselectAll()
                attributes = Rhino.DocObjects.ObjectAttributes()
                attributes.Name = "Viboceros Intersect Source"
                attributes.ObjectColor = System.Drawing.Color.FromArgb(12, 34, 56)
                attributes.ColorSource = Rhino.DocObjects.ObjectColorSource.ColorFromObject
                first_id = document.Objects.AddBrep(first, attributes)
                second_id = document.Objects.AddBrep(second, attributes)
                if first_id == System.Guid.Empty or second_id == System.Guid.Empty:
                    raise ValueError("could not add B-rep/B-rep Intersect inputs")
                if operation.get("reverse_selection", False):
                    input_ids.extend([second_id, first_id])
                else:
                    input_ids.extend([first_id, second_id])
                for object_id in input_ids:
                    document.Objects.Select(object_id)
                group_index = document.Groups.Add(
                    "Viboceros Intersect Group " + str(System.Guid.NewGuid()),
                    input_ids,
                )
                if group_index < 0:
                    raise ValueError("could not group B-rep/B-rep Intersect inputs")
                succeeded = Rhino.RhinoApp.RunScript("_-Intersect _Enter", False)
                records = []
                for item in document.Objects:
                    if item.Id in original_ids or item.Id in input_ids:
                        continue
                    geometry = item.Geometry
                    if isinstance(geometry, Rhino.Geometry.Point):
                        location = geometry.Location
                        value = {
                            "kind": "point",
                            "point": [
                                float(location.X),
                                float(location.Y),
                                float(location.Z),
                            ],
                        }
                        sort_key = (
                            "point",
                            float(location.X),
                            float(location.Y),
                            float(location.Z),
                        )
                    elif isinstance(geometry, Rhino.Geometry.Curve):
                        if operation.get("canonicalize_linear_curves", False):
                            definition = (
                                _canonical_linear_intersection_curve_definition(geometry)
                            )
                        else:
                            definition = _nurbs_curve_definition(geometry)
                        value = {"kind": "curve", "curve": definition}
                        sort_key = ("curve",) + tuple(
                            definition["control_points"][0]["point"]
                        )
                    else:
                        raise ValueError(
                            "B-rep/B-rep Intersect produced unsupported geometry %s"
                            % type(geometry).__name__
                        )
                    groups = item.Attributes.GetGroupList()
                    value.update({
                        "blank_name": not bool(item.Attributes.Name),
                        "color_from_layer": (
                            item.Attributes.ColorSource
                            == Rhino.DocObjects.ObjectColorSource.ColorFromLayer
                        ),
                        "in_source_group": (
                            groups is not None and group_index in groups
                        ),
                        "on_current_layer": (
                            int(item.Attributes.LayerIndex)
                            == int(document.Layers.CurrentLayerIndex)
                        ),
                        "selected": item.IsSelected(False) > 0,
                    })
                    records.append((sort_key, value))
                records.sort(key=lambda record: record[0])
                return {
                    "command_succeeded": bool(succeeded),
                    "input_selected": [
                        document.Objects.FindId(object_id).IsSelected(False) > 0
                        for object_id in input_ids
                    ],
                    "objects": [value for _, value in records],
                }
            finally:
                document.Objects.UnselectAll()
                for item in list(document.Objects):
                    if item.Id not in original_ids:
                        document.Objects.Delete(item.Id, True)
                if group_index >= 0 and not document.Groups.IsDeleted(group_index):
                    document.Groups.Delete(group_index)

        try:
            return _measure(iterations, intersect_breps_command)
        finally:
            first.Dispose()
            second.Dispose()

    if kind == "curve_extrude_command":
        document = Rhino.RhinoDoc.ActiveDoc
        source = _cut_source(operation["curve"])
        def extrude_native_curve():
            original_ids = set(item.Id for item in document.Objects)
            try:
                document.Objects.UnselectAll()
                source_id = document.Objects.AddCurve(source)
                document.Objects.Select(source_id)
                Rhino.RhinoApp.RunScript("_-CreaseSplitting _Disable", False)
                command = "_-ExtrudeCrv _Output=_Surface _Solid=_No %.17g" % float(operation["distance"])
                if not Rhino.RhinoApp.RunScript(command, False):
                    raise ValueError("native extrusion command failed")
                surfaces = []
                for item in document.Objects:
                    if item.Id == source_id or item.Id in original_ids:
                        continue
                    geometry = item.Geometry
                    if isinstance(geometry, Rhino.Geometry.Brep) and geometry.Faces.Count == 1:
                        geometry = geometry.Faces[0].UnderlyingSurface()
                    if not isinstance(geometry, Rhino.Geometry.Surface):
                        raise ValueError("native extrusion did not produce one surface")
                    surfaces.append(_nurbs_surface_definition(geometry))
                if not surfaces:
                    raise ValueError("native extrusion left no surfaces")
                return {"surfaces": surfaces}
            finally:
                Rhino.RhinoApp.RunScript("_-CreaseSplitting _Enable", False)
                document.Objects.UnselectAll()
                for item in list(document.Objects):
                    if item.Id not in original_ids:
                        document.Objects.Delete(item.Id, True)
        try:
            return _measure(iterations, extrude_native_curve)
        finally:
            source.Dispose()

    if kind == "curve_split_command":
        document = Rhino.RhinoDoc.ActiveDoc
        source = _cut_source(operation["curve"])
        cutters = []
        try:
            for definition in operation["cutters"]:
                cutters.append(
                    _curve_extension_boundary_from_definition(definition, tolerance)
                )
        except Exception:
            source.Dispose()
            for cutter in cutters:
                cutter.Dispose()
            raise

        def split_curve_command():
            original_ids = set(item.Id for item in document.Objects)
            source_id = System.Guid.Empty
            cutter_ids = []
            group_index = -1
            try:
                document.Objects.UnselectAll()
                attributes = Rhino.DocObjects.ObjectAttributes()
                attributes.Name = "Viboceros Split Source"
                attributes.ObjectColor = System.Drawing.Color.FromArgb(12, 34, 56)
                attributes.ColorSource = Rhino.DocObjects.ObjectColorSource.ColorFromObject
                source_id = document.Objects.AddCurve(source, attributes)
                if source_id == System.Guid.Empty:
                    raise ValueError("could not add Split command source curve")
                group_index = document.Groups.Add(
                    "Viboceros Split Group " + str(System.Guid.NewGuid()),
                    [source_id],
                )
                if group_index < 0:
                    raise ValueError("could not group Split command source curve")
                for cutter in cutters:
                    if isinstance(cutter, Rhino.Geometry.Curve):
                        cutter_id = document.Objects.AddCurve(cutter)
                    elif isinstance(cutter, Rhino.Geometry.Surface):
                        cutter_id = document.Objects.AddSurface(cutter)
                    elif isinstance(cutter, Rhino.Geometry.Brep):
                        cutter_id = document.Objects.AddBrep(cutter)
                    else:
                        raise ValueError("unsupported Split cutter geometry")
                    if cutter_id == System.Guid.Empty:
                        raise ValueError("could not add Split command cutter object")
                    cutter_ids.append(cutter_id)
                document.Objects.Select(source_id)
                command = "_-Split %s _Enter" % " ".join(
                    "_SelID %s" % str(cutter_id) for cutter_id in cutter_ids
                )
                succeeded = Rhino.RhinoApp.RunScript(command, False)
                objects = [
                    item
                    for item in document.Objects
                    if item.Id not in original_ids
                    and item.Id not in cutter_ids
                    and isinstance(item.Geometry, Rhino.Geometry.Curve)
                ]
                if not objects:
                    history = Rhino.RhinoApp.CommandHistoryWindowText
                    raise ValueError(
                        "Split macro %r returned %r and left no result curves; history tail: %s"
                        % (command, succeeded, history[-3000:])
                    )
                records = []
                for item in objects:
                    groups = item.Attributes.GetGroupList()
                    color = item.Attributes.ObjectColor
                    records.append({
                        "attributes_match_source": (
                            item.Attributes.Name == "Viboceros Split Source"
                            and int(item.Attributes.LayerIndex) == int(attributes.LayerIndex)
                            and int(color.R) == 12
                            and int(color.G) == 34
                            and int(color.B) == 56
                            and item.Attributes.ColorSource
                            == Rhino.DocObjects.ObjectColorSource.ColorFromObject
                        ),
                        "curve": _nurbs_curve_definition(item.Geometry),
                        "native": _cut_native_record(item.Geometry),
                        "in_source_group": (
                            groups is not None and group_index in groups
                        ),
                        "original_id": item.Id == source_id,
                        "selected": item.IsSelected(False) > 0,
                    })
                records.sort(
                    key=lambda record: tuple(
                        record["native"]["domain"]
                    )
                )
                return {
                    "command_succeeded": bool(succeeded),
                    "objects": records,
                }
            finally:
                document.Objects.UnselectAll()
                for item in list(document.Objects):
                    if item.Id not in original_ids:
                        document.Objects.Delete(item.Id, True)
                if group_index >= 0 and not document.Groups.IsDeleted(group_index):
                    document.Groups.Delete(group_index)

        try:
            return _measure(iterations, split_curve_command)
        finally:
            source.Dispose()
            for cutter in cutters:
                cutter.Dispose()

    if kind == "surface_split_cutting_command":
        document = Rhino.RhinoDoc.ActiveDoc
        source = _nurbs_surface_from_definition(operation["surface"])
        cutters = [
            _surface_split_cutter_from_definition(definition, tolerance)
            for definition in operation["cutters"]
        ]

        def split_surface_cutting_command():
            original_ids = set(item.Id for item in document.Objects)
            source_id = System.Guid.Empty
            cutter_ids = []
            group_index = -1
            try:
                document.Objects.UnselectAll()
                attributes = Rhino.DocObjects.ObjectAttributes()
                attributes.Name = "Viboceros Split Surface Source"
                attributes.ObjectColor = System.Drawing.Color.FromArgb(12, 34, 56)
                attributes.ColorSource = Rhino.DocObjects.ObjectColorSource.ColorFromObject
                source_id = document.Objects.AddSurface(source, attributes)
                if source_id == System.Guid.Empty:
                    raise ValueError("could not add cutting Split source surface")
                group_index = document.Groups.Add(
                    "Viboceros Split Surface Group " + str(System.Guid.NewGuid()),
                    [source_id],
                )
                if group_index < 0:
                    raise ValueError("could not group cutting Split source surface")
                document.Objects.Select(source_id)
                pretrim = operation.get("pretrim")
                if pretrim is not None:
                    pretrim_direction = str(pretrim["direction"]).lower()
                    if pretrim_direction not in ("u", "v", "both"):
                        raise ValueError(
                            "surface cutting Split pretrim direction must be u, v, or both"
                        )
                    pretrim_command = (
                        "_-Split _Isocurve _Direction=_%s _Shrink=_No %s _Enter"
                        % (
                            pretrim_direction.upper()
                            if pretrim_direction != "both"
                            else "Both",
                            _command_point(pretrim["point"]),
                        )
                    )
                    pretrim_succeeded = Rhino.RhinoApp.RunScript(
                        pretrim_command, False
                    )
                    pretrim_pieces = []
                    for item in document.Objects:
                        if item.Id in original_ids:
                            continue
                        geometry = item.Geometry
                        if not (
                            isinstance(geometry, Rhino.Geometry.Brep)
                            and geometry.Faces.Count == 1
                        ):
                            continue
                        face = geometry.Faces[0]
                        trim_points = []
                        for trim in face.OuterLoop.Trims:
                            trim_points.extend([trim.PointAtStart, trim.PointAtEnd])
                        if not trim_points:
                            continue
                        bounds = [
                            min(float(point.X) for point in trim_points),
                            max(float(point.X) for point in trim_points),
                            min(float(point.Y) for point in trim_points),
                            max(float(point.Y) for point in trim_points),
                        ]
                        pretrim_pieces.append((bounds, item.Id))
                    pretrim_pieces.sort(
                        key=lambda piece: (
                            piece[0][0],
                            piece[0][2],
                            piece[0][1],
                            piece[0][3],
                        )
                    )
                    expected_pretrim_count = (
                        4 if pretrim_direction == "both" else 2
                    )
                    piece_index = int(pretrim["piece"])
                    if (
                        not pretrim_succeeded
                        or len(pretrim_pieces) != expected_pretrim_count
                        or piece_index < 0
                        or piece_index >= len(pretrim_pieces)
                    ):
                        raise ValueError(
                            "surface cutting Split pretrim macro %r returned %r and left %d "
                            "rectangular pieces; expected %d with retained index %d"
                            % (
                                pretrim_command,
                                pretrim_succeeded,
                                len(pretrim_pieces),
                                expected_pretrim_count,
                                piece_index,
                            )
                        )
                    source_id = pretrim_pieces[piece_index][1]
                    for _bounds, piece_id in pretrim_pieces:
                        if piece_id != source_id:
                            document.Objects.Delete(piece_id, True)
                    document.Objects.UnselectAll()
                    document.Objects.Select(source_id)
                for cutter in cutters:
                    if isinstance(cutter, Rhino.Geometry.Curve):
                        cutter_id = document.Objects.AddCurve(cutter)
                    elif isinstance(cutter, Rhino.Geometry.Surface):
                        cutter_id = document.Objects.AddSurface(cutter)
                    elif isinstance(cutter, Rhino.Geometry.Brep):
                        cutter_id = document.Objects.AddBrep(cutter)
                    else:
                        raise ValueError("unsupported cutting Split cutter geometry")
                    if cutter_id == System.Guid.Empty:
                        raise ValueError("could not add cutting Split cutter")
                    cutter_ids.append(cutter_id)
                document.Objects.Select(source_id)
                command = "_-Split %s _Enter" % " ".join(
                    "_SelID %s" % str(cutter_id) for cutter_id in cutter_ids
                )
                succeeded = Rhino.RhinoApp.RunScript(command, False)
                records = []
                for item in document.Objects:
                    if item.Id in original_ids or item.Id in cutter_ids:
                        continue
                    geometry = item.Geometry
                    if isinstance(geometry, Rhino.Geometry.Surface):
                        surface_geometry = geometry
                        object_kind = "surface"
                        topology = None
                        trim_curves = []
                        trim_bounds = [
                            float(geometry.Domain(0).T0),
                            float(geometry.Domain(0).T1),
                            float(geometry.Domain(1).T0),
                            float(geometry.Domain(1).T1),
                        ]
                    elif (
                        isinstance(geometry, Rhino.Geometry.Brep)
                        and geometry.Faces.Count == 1
                    ):
                        face = geometry.Faces[0]
                        surface_geometry = face.UnderlyingSurface()
                        topology = _mesh_to_nurb_brep_value(geometry)
                        object_kind = "brep"
                        trim_curves = [
                            _surface_split_trim_value(
                                trim, surface_geometry,
                                operation.get("sample_trim_geometry", False),
                            )
                            for brep_face in geometry.Faces
                            for loop in brep_face.Loops
                            for trim in loop.Trims
                            if str(trim.IsoStatus) == "None"
                        ]
                        trim_points = []
                        for trim in face.OuterLoop.Trims:
                            trim_points.extend([trim.PointAtStart, trim.PointAtEnd])
                        if not trim_points:
                            raise ValueError(
                                "cutting Split returned an empty outer trim loop"
                            )
                        trim_bounds = [
                            min(float(point.X) for point in trim_points),
                            max(float(point.X) for point in trim_points),
                            min(float(point.Y) for point in trim_points),
                            max(float(point.Y) for point in trim_points),
                        ]
                    else:
                        continue
                    groups = item.Attributes.GetGroupList()
                    color = item.Attributes.ObjectColor
                    records.append({
                        "attributes_match_source": (
                            item.Attributes.Name == "Viboceros Split Surface Source"
                            and int(item.Attributes.LayerIndex)
                            == int(attributes.LayerIndex)
                            and int(color.R) == 12
                            and int(color.G) == 34
                            and int(color.B) == 56
                            and item.Attributes.ColorSource
                            == Rhino.DocObjects.ObjectColorSource.ColorFromObject
                        ),
                        "in_source_group": (
                            groups is not None and group_index in groups
                        ),
                        "object_kind": object_kind,
                        "original_id": item.Id == source_id,
                        "selected": item.IsSelected(False) > 0,
                        "surface": _nurbs_surface_definition(surface_geometry),
                        "topology": topology,
                        "trim_curves": trim_curves,
                        "trim_bounds": trim_bounds,
                    })
                if not records:
                    history = Rhino.RhinoApp.CommandHistoryWindowText
                    raise ValueError(
                        "surface cutting Split macro %r returned %r and left no "
                        "surface pieces; history tail: %s"
                        % (command, succeeded, history[-3000:])
                    )
                records.sort(key=lambda record: (
                    record["trim_bounds"][0],
                    record["trim_bounds"][2],
                    record["trim_bounds"][1],
                    record["trim_bounds"][3],
                ))
                return {
                    "command_succeeded": bool(succeeded),
                    "cutters_selected": [
                        document.Objects.FindId(cutter_id).IsSelected(False) > 0
                        for cutter_id in cutter_ids
                    ],
                    "objects": records,
                }
            finally:
                document.Objects.UnselectAll()
                for item in list(document.Objects):
                    if item.Id not in original_ids:
                        document.Objects.Delete(item.Id, True)
                if group_index >= 0 and not document.Groups.IsDeleted(group_index):
                    document.Groups.Delete(group_index)

        try:
            return _measure(iterations, split_surface_cutting_command)
        finally:
            source.Dispose()
            for cutter in cutters:
                cutter.Dispose()

    if kind == "surface_split_isocurve_command":
        document = Rhino.RhinoDoc.ActiveDoc
        source = _nurbs_surface_from_definition(operation["surface"])
        direction = str(operation["direction"]).lower()
        if direction not in ("u", "v", "both"):
            source.Dispose()
            raise ValueError("surface Split direction must be u, v, or both")
        shrink = bool(operation.get("shrink", True))
        point = _command_point(operation["point"])

        def split_surface_isocurve_command():
            original_ids = set(item.Id for item in document.Objects)
            source_id = System.Guid.Empty
            group_index = -1
            try:
                document.Objects.UnselectAll()
                attributes = Rhino.DocObjects.ObjectAttributes()
                attributes.Name = "Viboceros Split Surface Source"
                attributes.ObjectColor = System.Drawing.Color.FromArgb(12, 34, 56)
                attributes.ColorSource = Rhino.DocObjects.ObjectColorSource.ColorFromObject
                source_id = document.Objects.AddSurface(source, attributes)
                if source_id == System.Guid.Empty:
                    raise ValueError("could not add Split command source surface")
                group_index = document.Groups.Add(
                    "Viboceros Split Surface Group " + str(System.Guid.NewGuid()),
                    [source_id],
                )
                if group_index < 0:
                    raise ValueError("could not group Split command source surface")
                document.Objects.Select(source_id)
                pretrim = operation.get("pretrim")
                if pretrim is not None:
                    pretrim_direction = str(pretrim["direction"]).lower()
                    if pretrim_direction not in ("u", "v", "both"):
                        raise ValueError(
                            "surface Split pretrim direction must be u, v, or both"
                        )
                    pretrim_command = (
                        "_-Split _Isocurve _Direction=_%s _Shrink=_No %s _Enter"
                        % (
                            pretrim_direction.upper()
                            if pretrim_direction != "both"
                            else "Both",
                            _command_point(pretrim["point"]),
                        )
                    )
                    pretrim_succeeded = Rhino.RhinoApp.RunScript(
                        pretrim_command, False
                    )
                    pretrim_pieces = []
                    for item in document.Objects:
                        if item.Id in original_ids:
                            continue
                        geometry = item.Geometry
                        if not (
                            isinstance(geometry, Rhino.Geometry.Brep)
                            and geometry.Faces.Count == 1
                        ):
                            continue
                        face = geometry.Faces[0]
                        trim_points = []
                        for trim in face.OuterLoop.Trims:
                            trim_points.extend(
                                [trim.PointAtStart, trim.PointAtEnd]
                            )
                        if not trim_points:
                            continue
                        bounds = [
                            min(float(point.X) for point in trim_points),
                            max(float(point.X) for point in trim_points),
                            min(float(point.Y) for point in trim_points),
                            max(float(point.Y) for point in trim_points),
                        ]
                        pretrim_pieces.append((bounds, item.Id))
                    pretrim_pieces.sort(
                        key=lambda piece: (
                            piece[0][0],
                            piece[0][2],
                            piece[0][1],
                            piece[0][3],
                        )
                    )
                    expected_pretrim_count = (
                        4 if pretrim_direction == "both" else 2
                    )
                    piece_index = int(pretrim["piece"])
                    if (
                        not pretrim_succeeded
                        or len(pretrim_pieces) != expected_pretrim_count
                        or piece_index < 0
                        or piece_index >= len(pretrim_pieces)
                    ):
                        raise ValueError(
                            "surface Split pretrim macro %r returned %r and left %d "
                            "rectangular pieces; expected %d with retained index %d"
                            % (
                                pretrim_command,
                                pretrim_succeeded,
                                len(pretrim_pieces),
                                expected_pretrim_count,
                                piece_index,
                            )
                        )
                    source_id = pretrim_pieces[piece_index][1]
                    for _bounds, piece_id in pretrim_pieces:
                        if piece_id != source_id:
                            document.Objects.Delete(piece_id, True)
                    document.Objects.UnselectAll()
                    document.Objects.Select(source_id)
                command = "_-Split _Isocurve _Direction=_%s _Shrink=_%s %s _Enter" % (
                    direction.upper() if direction != "both" else "Both",
                    "Yes" if shrink else "No",
                    point,
                )
                succeeded = Rhino.RhinoApp.RunScript(command, False)
                objects = []
                unexpected_geometry_types = []
                for item in document.Objects:
                    if item.Id in original_ids:
                        continue
                    geometry = item.Geometry
                    if isinstance(geometry, Rhino.Geometry.Surface):
                        surface_geometry = geometry
                        object_kind = "surface"
                        topology = None
                        trim_bounds = [
                            float(geometry.Domain(0).T0),
                            float(geometry.Domain(0).T1),
                            float(geometry.Domain(1).T0),
                            float(geometry.Domain(1).T1),
                        ]
                    elif (
                        isinstance(geometry, Rhino.Geometry.Brep)
                        and geometry.Faces.Count == 1
                    ):
                        face = geometry.Faces[0]
                        surface_geometry = face.UnderlyingSurface()
                        object_kind = "brep"
                        topology = _mesh_to_nurb_brep_value(geometry)
                        trim_points = []
                        for trim in face.OuterLoop.Trims:
                            trim_points.extend([trim.PointAtStart, trim.PointAtEnd])
                        if not trim_points:
                            raise ValueError("surface Split returned an empty outer trim loop")
                        trim_bounds = [
                            min(float(point.X) for point in trim_points),
                            max(float(point.X) for point in trim_points),
                            min(float(point.Y) for point in trim_points),
                            max(float(point.Y) for point in trim_points),
                        ]
                    else:
                        if isinstance(geometry, Rhino.Geometry.Brep):
                            unexpected_geometry_types.append(
                                "%s(faces=%d, loops=%d, trims=%d, edges=%d, vertices=%d)"
                                % (
                                    geometry.GetType().FullName,
                                    geometry.Faces.Count,
                                    geometry.Loops.Count,
                                    geometry.Trims.Count,
                                    geometry.Edges.Count,
                                    geometry.Vertices.Count,
                                )
                            )
                        else:
                            unexpected_geometry_types.append(
                                geometry.GetType().FullName
                            )
                        continue
                    definition = _nurbs_surface_definition(surface_geometry)
                    objects.append(
                        (item, definition, object_kind, trim_bounds, topology)
                    )
                expected_count = 4 if direction == "both" else 2
                if len(objects) != expected_count:
                    history = Rhino.RhinoApp.CommandHistoryWindowText
                    raise ValueError(
                        "surface Split macro %r returned %r and left %d surface objects; "
                        "expected %d; unexpected geometry types: %r; history tail: %s"
                        % (
                            command,
                            succeeded,
                            len(objects),
                            expected_count,
                            unexpected_geometry_types,
                            history[-3000:],
                        )
                    )
                records = []
                for item, definition, object_kind, trim_bounds, topology in objects:
                    groups = item.Attributes.GetGroupList()
                    color = item.Attributes.ObjectColor
                    records.append({
                        "attributes_match_source": (
                            item.Attributes.Name == "Viboceros Split Surface Source"
                            and int(item.Attributes.LayerIndex) == int(attributes.LayerIndex)
                            and int(color.R) == 12
                            and int(color.G) == 34
                            and int(color.B) == 56
                            and item.Attributes.ColorSource
                            == Rhino.DocObjects.ObjectColorSource.ColorFromObject
                        ),
                        "in_source_group": (
                            groups is not None and group_index in groups
                        ),
                        "object_kind": object_kind,
                        "original_id": item.Id == source_id,
                        "selected": item.IsSelected(False) > 0,
                        "surface": definition,
                        "topology": topology,
                        "trim_bounds": trim_bounds,
                    })
                records.sort(key=lambda record: (
                    record["trim_bounds"][0],
                    record["trim_bounds"][2],
                    record["trim_bounds"][1],
                    record["trim_bounds"][3],
                ))
                return {
                    "command_succeeded": bool(succeeded),
                    "objects": records,
                }
            finally:
                document.Objects.UnselectAll()
                for item in list(document.Objects):
                    if item.Id not in original_ids:
                        document.Objects.Delete(item.Id, True)
                if group_index >= 0 and not document.Groups.IsDeleted(group_index):
                    document.Groups.Delete(group_index)

        try:
            return _measure(iterations, split_surface_isocurve_command)
        finally:
            source.Dispose()

    if kind == "curve_trim_command":
        document = Rhino.RhinoDoc.ActiveDoc
        source = _cut_source(operation["curve"])
        cutters = []
        try:
            for definition in operation["cutters"]:
                cutters.append(
                    _curve_extension_boundary_from_definition(definition, tolerance)
                )
        except Exception:
            source.Dispose()
            for cutter in cutters:
                cutter.Dispose()
            raise
        pick = _point(operation["pick"])

        def crossing_selection(point, radius):
            return "_SelCrossing %.17g,%.17g %.17g,%.17g" % (
                point.X - radius,
                point.Y - radius,
                point.X + radius,
                point.Y + radius,
            )

        def trim_curve_command():
            original_ids = set(item.Id for item in document.Objects)
            source_id = System.Guid.Empty
            cutter_ids = []
            group_index = -1
            try:
                document.Objects.UnselectAll()
                attributes = Rhino.DocObjects.ObjectAttributes()
                attributes.Name = "Viboceros Trim Source"
                attributes.ObjectColor = System.Drawing.Color.FromArgb(12, 34, 56)
                attributes.ColorSource = Rhino.DocObjects.ObjectColorSource.ColorFromObject
                source_id = document.Objects.AddCurve(source, attributes)
                if source_id == System.Guid.Empty:
                    raise ValueError("could not add Trim command source curve")
                group_index = document.Groups.Add(
                    "Viboceros Trim Group " + str(System.Guid.NewGuid()),
                    [source_id],
                )
                if group_index < 0:
                    raise ValueError("could not group Trim command source curve")
                for cutter in cutters:
                    if isinstance(cutter, Rhino.Geometry.Curve):
                        cutter_id = document.Objects.AddCurve(cutter)
                    elif isinstance(cutter, Rhino.Geometry.Surface):
                        cutter_id = document.Objects.AddSurface(cutter)
                    elif isinstance(cutter, Rhino.Geometry.Brep):
                        cutter_id = document.Objects.AddBrep(cutter)
                    else:
                        raise ValueError("unsupported Trim cutter geometry")
                    if cutter_id == System.Guid.Empty:
                        raise ValueError("could not add Trim command cutter object")
                    cutter_ids.append(cutter_id)
                    document.Objects.Select(cutter_id)
                Rhino.RhinoApp.RunScript("_-SetView _World _Top _Zoom _Extents", False)
                radius = max(1.0, float(source.GetBoundingBox(True).Diagonal.Length)) * 0.02
                apparent = bool(operation.get("apparent_intersections", True))
                command = "_-Trim _ApparentIntersections=_%s %s _Enter" % (
                    "Yes" if apparent else "No",
                    crossing_selection(pick, radius),
                )
                succeeded = Rhino.RhinoApp.RunScript(command, False)
                objects = [
                    item
                    for item in document.Objects
                    if item.Id not in original_ids
                    and item.Id not in cutter_ids
                    and isinstance(item.Geometry, Rhino.Geometry.Curve)
                ]
                if not objects:
                    history = Rhino.RhinoApp.CommandHistoryWindowText
                    raise ValueError(
                        "Trim macro %r returned %r and left no result curves; history tail: %s"
                        % (command, succeeded, history[-3000:])
                    )
                records = []
                for item in objects:
                    groups = item.Attributes.GetGroupList()
                    color = item.Attributes.ObjectColor
                    records.append({
                        "attributes_match_source": (
                            item.Attributes.Name == "Viboceros Trim Source"
                            and int(item.Attributes.LayerIndex) == int(attributes.LayerIndex)
                            and int(color.R) == 12
                            and int(color.G) == 34
                            and int(color.B) == 56
                            and item.Attributes.ColorSource
                            == Rhino.DocObjects.ObjectColorSource.ColorFromObject
                        ),
                        "curve": _nurbs_curve_definition(item.Geometry),
                        "native": _cut_native_record(item.Geometry),
                        "in_source_group": (
                            groups is not None and group_index in groups
                        ),
                        "original_id": item.Id == source_id,
                        "selected": item.IsSelected(False) > 0,
                    })
                records.sort(
                    key=lambda record: tuple(
                        record["native"]["domain"]
                    )
                )
                return {
                    "command_succeeded": bool(succeeded),
                    "objects": records,
                }
            finally:
                document.Objects.UnselectAll()
                for item in list(document.Objects):
                    if item.Id not in original_ids:
                        document.Objects.Delete(item.Id, True)
                if group_index >= 0 and not document.Groups.IsDeleted(group_index):
                    document.Groups.Delete(group_index)

        try:
            return _measure(iterations, trim_curve_command)
        finally:
            source.Dispose()
            for cutter in cutters:
                cutter.Dispose()

    if kind == "curve_split_geometry":
        source = _nurbs_curve_from_definition(operation["curve"])
        parameter = _finite(operation["parameter"], "curve split parameter")

        def split_curve():
            pieces = source.Split(parameter)
            if pieces is None or len(pieces) != 2:
                if pieces is not None:
                    for piece in pieces:
                        piece.Dispose()
                raise ValueError("Rhino curve split failed")
            try:
                definitions = []
                for piece in pieces:
                    definition = _nurbs_curve_definition(piece)
                    definition["closed"] = bool(piece.IsClosed)
                    definition["periodic"] = bool(piece.IsPeriodic)
                    definitions.append(definition)
                return definitions
            finally:
                for piece in pieces:
                    piece.Dispose()

        try:
            return _measure(iterations, split_curve)
        finally:
            source.Dispose()

    if kind == "curve_multi_split_geometry":
        source = _nurbs_curve_from_definition(operation["curve"])
        parameters = System.Array[System.Double](
            [
                _finite(parameter, "curve split parameter")
                for parameter in operation["parameters"]
            ]
        )

        def split_curve_multiple():
            pieces = source.Split(parameters)
            if pieces is None:
                raise ValueError("Rhino multiple curve split failed")
            try:
                definitions = []
                for piece in pieces:
                    definition = _nurbs_curve_definition(piece)
                    definition["closed"] = bool(piece.IsClosed)
                    definition["periodic"] = bool(piece.IsPeriodic)
                    definitions.append(definition)
                return definitions
            finally:
                for piece in pieces:
                    piece.Dispose()

        try:
            return _measure(iterations, split_curve_multiple)
        finally:
            source.Dispose()

    if kind == "surface_change_seam_geometry":
        degree_u = int(operation["degree_u"])
        degree_v = int(operation["degree_v"])
        count_u = int(operation["control_point_count_u"])
        count_v = int(operation["control_point_count_v"])
        direction = str(operation["direction"]).lower()
        if direction not in ("u", "v", "both"):
            raise ValueError("surface seam direction must be u, v, or both")
        parameters = operation["parameter"]
        if len(parameters) != 2:
            raise ValueError("surface seam relocation requires a u,v parameter")
        parameter_u = _finite(parameters[0], "surface U seam parameter")
        parameter_v = _finite(parameters[1], "surface V seam parameter")
        source = Rhino.Geometry.NurbsSurface.Create(
            3, True, degree_u + 1, degree_v + 1, count_u, count_v
        )
        if source is None:
            raise ValueError("could not allocate surface seam source")
        try:
            _set_surface_controls(
                source, operation["control_points"], count_u, count_v
            )
            _set_knots(source.KnotsU, operation["knots_u"], "surface U knot")
            _set_knots(source.KnotsV, operation["knots_v"], "surface V knot")
            if not source.IsValid:
                raise ValueError("surface seam source is invalid")
        except Exception:
            source.Dispose()
            raise

        def change_surface_seam():
            current = source.ToBrep()
            if current is None or current.Faces.Count != 1:
                raise ValueError("could not create a one-face surface seam B-rep")
            try:
                axes = []
                if direction in ("u", "both"):
                    axes.append((0, parameter_u))
                if direction in ("v", "both"):
                    axes.append((1, parameter_v))
                for axis, parameter in axes:
                    changed = Rhino.Geometry.Brep.ChangeSeam(
                        current.Faces[0], axis, parameter, tolerance["absolute"]
                    )
                    if changed is None:
                        raise ValueError("Rhino surface seam relocation failed")
                    current.Dispose()
                    current = changed
                nurbs = current.Faces[0].UnderlyingSurface().ToNurbsSurface()
                if nurbs is None:
                    raise ValueError("surface seam relocation returned no NURBS surface")
                try:
                    definition = _nurbs_surface_definition(nurbs)
                    definition["periodic_u"] = bool(nurbs.IsPeriodic(0))
                    definition["periodic_v"] = bool(nurbs.IsPeriodic(1))
                    return definition
                finally:
                    nurbs.Dispose()
            finally:
                current.Dispose()

        try:
            return _measure(iterations, change_surface_seam)
        finally:
            source.Dispose()

    if kind == "surface_reparameterize_geometry":
        degree_u = int(operation["degree_u"])
        degree_v = int(operation["degree_v"])
        count_u = int(operation["control_point_count_u"])
        count_v = int(operation["control_point_count_v"])
        source = Rhino.Geometry.NurbsSurface.Create(
            3, True, degree_u + 1, degree_v + 1, count_u, count_v
        )
        if source is None:
            raise ValueError("could not allocate surface reparameterization source")
        try:
            _set_surface_controls(
                source, operation["control_points"], count_u, count_v
            )
            _set_knots(source.KnotsU, operation["knots_u"], "surface U knot")
            _set_knots(source.KnotsV, operation["knots_v"], "surface V knot")
            if not source.IsValid:
                raise ValueError("surface reparameterization source is invalid")
            domain_u = operation.get("domain_u")
            domain_v = operation.get("domain_v")
            if domain_u is None and domain_v is None:
                sized, width, height = source.GetSurfaceSize()
                if not sized:
                    raise ValueError("Rhino could not estimate the NURBS surface size")
                domain_u = [0.0, float(width)]
                domain_v = [0.0, float(height)]
            elif domain_u is None or domain_v is None:
                raise ValueError(
                    "surface reparameterization requires both domains or neither"
                )
            if len(domain_u) != 2 or len(domain_v) != 2:
                raise ValueError("surface reparameterization requires U and V domains")
            targets = [
                Rhino.Geometry.Interval(
                    _finite(domain_u[0], "surface U domain start"),
                    _finite(domain_u[1], "surface U domain end"),
                ),
                Rhino.Geometry.Interval(
                    _finite(domain_v[0], "surface V domain start"),
                    _finite(domain_v[1], "surface V domain end"),
                ),
            ]
            if not all(target.IsIncreasing for target in targets):
                raise ValueError("surface reparameterization domains must be increasing")
        except Exception:
            source.Dispose()
            raise

        def reparameterize_surface():
            duplicate = source.Duplicate()
            if duplicate is None or not isinstance(
                duplicate, Rhino.Geometry.NurbsSurface
            ):
                raise ValueError("Rhino could not duplicate surface for reparameterization")
            try:
                for axis, target in enumerate(targets):
                    if not duplicate.SetDomain(axis, target):
                        raise ValueError("Rhino surface reparameterization failed")
                definition = _nurbs_surface_definition(duplicate)
                definition["periodic_u"] = bool(duplicate.IsPeriodic(0))
                definition["periodic_v"] = bool(duplicate.IsPeriodic(1))
                return definition
            finally:
                duplicate.Dispose()

        try:
            return _measure(iterations, reparameterize_surface)
        finally:
            source.Dispose()

    if kind == "surface_extend_geometry":
        degree_u = int(operation["degree_u"])
        degree_v = int(operation["degree_v"])
        count_u = int(operation["control_point_count_u"])
        count_v = int(operation["control_point_count_v"])
        direction = str(operation["direction"]).lower()
        if direction not in ("u", "v"):
            raise ValueError("surface extension direction must be u or v")
        values = operation["domain"]
        if len(values) != 2:
            raise ValueError("surface extension domain requires two parameters")
        target = Rhino.Geometry.Interval(
            _finite(values[0], "surface extension domain start"),
            _finite(values[1], "surface extension domain end"),
        )
        if not target.IsIncreasing:
            raise ValueError("surface extension domain must be increasing")
        source = Rhino.Geometry.NurbsSurface.Create(
            3, True, degree_u + 1, degree_v + 1, count_u, count_v
        )
        if source is None:
            raise ValueError("could not allocate surface extension source")
        try:
            _set_surface_controls(
                source, operation["control_points"], count_u, count_v
            )
            _set_knots(source.KnotsU, operation["knots_u"], "surface U knot")
            _set_knots(source.KnotsV, operation["knots_v"], "surface V knot")
            if not source.IsValid:
                raise ValueError("surface extension source is invalid")
        except Exception:
            source.Dispose()
            raise

        def extend_surface():
            duplicate = source.Duplicate()
            if duplicate is None or not isinstance(
                duplicate, Rhino.Geometry.NurbsSurface
            ):
                raise ValueError("Rhino could not duplicate surface for extension")
            try:
                axis = 0 if direction == "u" else 1
                if not duplicate.Extend(axis, target):
                    raise ValueError("Rhino natural surface extension failed")
                definition = _nurbs_surface_definition(duplicate)
                definition["periodic_u"] = bool(duplicate.IsPeriodic(0))
                definition["periodic_v"] = bool(duplicate.IsPeriodic(1))
                return definition
            finally:
                duplicate.Dispose()

        try:
            return _measure(iterations, extend_surface)
        finally:
            source.Dispose()

    if kind == "surface_extend_length_geometry":
        degree_u = int(operation["degree_u"])
        degree_v = int(operation["degree_v"])
        count_u = int(operation["control_point_count_u"])
        count_v = int(operation["control_point_count_v"])
        edge_name = str(operation["edge"]).lower()
        edges = {
            "west": Rhino.Geometry.IsoStatus.West,
            "south": Rhino.Geometry.IsoStatus.South,
            "east": Rhino.Geometry.IsoStatus.East,
            "north": Rhino.Geometry.IsoStatus.North,
        }
        if edge_name not in edges:
            raise ValueError("surface extension edge must be west, south, east, or north")
        length = _finite(operation["length"], "surface extension length")
        smooth = bool(operation.get("smooth", True))
        source = Rhino.Geometry.NurbsSurface.Create(
            3, True, degree_u + 1, degree_v + 1, count_u, count_v
        )
        if source is None:
            raise ValueError("could not allocate surface length-extension source")
        try:
            _set_surface_controls(
                source, operation["control_points"], count_u, count_v
            )
            _set_knots(source.KnotsU, operation["knots_u"], "surface U knot")
            _set_knots(source.KnotsV, operation["knots_v"], "surface V knot")
            if not source.IsValid:
                raise ValueError("surface length-extension source is invalid")
        except Exception:
            source.Dispose()
            raise

        def extend_surface_by_length():
            if length < 0.0:
                document = Rhino.RhinoDoc.ActiveDoc
                object_id = System.Guid.Empty
                nurbs = None
                try:
                    document.Objects.UnselectAll()
                    object_id = document.Objects.AddSurface(source)
                    if object_id == System.Guid.Empty:
                        raise ValueError("could not add surface shrink source")
                    surface_object = document.Objects.FindId(object_id)
                    brep = surface_object.Geometry
                    edge_index = None
                    if (
                        isinstance(brep, Rhino.Geometry.Brep)
                        and brep.Faces.Count == 1
                    ):
                        for trim in brep.Faces[0].OuterLoop.Trims:
                            if (
                                trim.IsoStatus == edges[edge_name]
                                and trim.Edge is not None
                            ):
                                edge_index = int(trim.Edge.EdgeIndex)
                                break
                    if edge_index is None:
                        raise ValueError("could not locate natural surface shrink edge")
                    component = Rhino.Geometry.ComponentIndex(
                        Rhino.Geometry.ComponentIndexType.BrepEdge,
                        edge_index,
                    )
                    if surface_object.SelectSubObject(
                        component, True, True, False
                    ) == 0:
                        raise ValueError("could not select surface shrink edge")
                    command = "_-ExtendSrf _Type=_%s _Merge=_Yes %.17g _Enter" % (
                        "Smooth" if smooth else "Line",
                        length,
                    )
                    Rhino.RhinoApp.RunScript(command, False)
                    result_object = document.Objects.FindId(object_id)
                    if result_object is None:
                        raise ValueError("surface shrink removed its source")
                    geometry = result_object.Geometry
                    if isinstance(geometry, Rhino.Geometry.Brep):
                        if geometry.Faces.Count != 1:
                            raise ValueError("surface shrink returned a polysurface")
                        nurbs = geometry.Faces[0].UnderlyingSurface().ToNurbsSurface()
                    else:
                        nurbs = geometry.ToNurbsSurface()
                    if nurbs is None:
                        raise ValueError("surface shrink returned no NURBS surface")
                    definition = _nurbs_surface_definition(nurbs)
                    definition["periodic_u"] = bool(nurbs.IsPeriodic(0))
                    definition["periodic_v"] = bool(nurbs.IsPeriodic(1))
                    return definition
                finally:
                    if nurbs is not None:
                        nurbs.Dispose()
                    document.Objects.UnselectAll()
                    if document.Objects.FindId(object_id) is not None:
                        document.Objects.Delete(object_id, True)

            result = source.Extend(edges[edge_name], length, smooth)
            if result is None:
                raise ValueError("Rhino surface length extension failed")
            try:
                nurbs = result.ToNurbsSurface()
                if nurbs is None:
                    raise ValueError("surface length extension returned no NURBS surface")
                try:
                    definition = _nurbs_surface_definition(nurbs)
                    definition["periodic_u"] = bool(nurbs.IsPeriodic(0))
                    definition["periodic_v"] = bool(nurbs.IsPeriodic(1))
                    return definition
                finally:
                    nurbs.Dispose()
            finally:
                result.Dispose()

        try:
            return _measure(iterations, extend_surface_by_length)
        finally:
            source.Dispose()

    if kind == "surface_direction_edit_geometry":
        degree_u = int(operation["degree_u"])
        degree_v = int(operation["degree_v"])
        count_u = int(operation["control_point_count_u"])
        count_v = int(operation["control_point_count_v"])
        edit = str(operation["edit"]).lower()
        if edit not in ("u_reverse", "v_reverse", "swap_uv"):
            raise ValueError("surface direction edit must be u_reverse, v_reverse, or swap_uv")
        source = Rhino.Geometry.NurbsSurface.Create(
            3, True, degree_u + 1, degree_v + 1, count_u, count_v
        )
        if source is None:
            raise ValueError("could not allocate surface direction source")
        try:
            _set_surface_controls(
                source, operation["control_points"], count_u, count_v
            )
            _set_knots(source.KnotsU, operation["knots_u"], "surface U knot")
            _set_knots(source.KnotsV, operation["knots_v"], "surface V knot")
            if not source.IsValid:
                raise ValueError("surface direction source is invalid")
        except Exception:
            source.Dispose()
            raise

        def edit_surface_direction():
            if edit == "u_reverse":
                result = source.Reverse(0)
            elif edit == "v_reverse":
                result = source.Reverse(1)
            else:
                result = source.Transpose()
            if result is None:
                raise ValueError("Rhino surface direction edit failed")
            try:
                nurbs = result.ToNurbsSurface()
                if nurbs is None:
                    raise ValueError("surface direction edit returned no NURBS surface")
                try:
                    definition = _nurbs_surface_definition(nurbs)
                    definition["periodic_u"] = bool(nurbs.IsPeriodic(0))
                    definition["periodic_v"] = bool(nurbs.IsPeriodic(1))
                    return definition
                finally:
                    nurbs.Dispose()
            finally:
                result.Dispose()

        try:
            return _measure(iterations, edit_surface_direction)
        finally:
            source.Dispose()

    if kind == "curve_insert_control_point_geometry":
        document = Rhino.RhinoDoc.ActiveDoc
        source = _nurbs_curve_from_definition(operation["curve"])
        parameter = _finite(
            operation["parameter"], "curve control-point insertion parameter"
        )
        midpoint = bool(operation.get("midpoint", False))
        point = source.PointAt(parameter)
        command = "_InsertControlPoint _Midpoint=_%s %s _Enter" % (
            "Yes" if midpoint else "No",
            _command_point(_xyz(point)),
        )

        def insert_curve_control_point():
            object_id = System.Guid.Empty
            try:
                document.Objects.UnselectAll()
                object_id = document.Objects.AddCurve(source)
                if object_id == System.Guid.Empty:
                    raise ValueError("could not add InsertControlPoint source curve")
                document.Objects.Select(object_id, True)
                if not Rhino.RhinoApp.RunScript(command, False):
                    history = Rhino.RhinoApp.CommandHistoryWindowText
                    raise ValueError(
                        "Rhino curve control-point insertion failed; history tail: %s"
                        % history[-3000:]
                    )
                rhino_object = document.Objects.FindId(object_id)
                if rhino_object is None:
                    raise ValueError("InsertControlPoint removed the curve")
                result = rhino_object.Geometry.ToNurbsCurve()
                if result is None:
                    raise ValueError("InsertControlPoint returned no NURBS curve")
                try:
                    definition = _nurbs_curve_definition(result)
                    definition["closed"] = bool(result.IsClosed)
                    definition["periodic"] = bool(result.IsPeriodic)
                    return definition
                finally:
                    result.Dispose()
            finally:
                existing = document.Objects.FindId(object_id)
                if existing is not None:
                    document.Objects.Delete(object_id, True)

        try:
            return _measure(iterations, insert_curve_control_point)
        finally:
            source.Dispose()

    if kind == "surface_insert_control_point_geometry":
        document = Rhino.RhinoDoc.ActiveDoc
        degree_u = int(operation["degree_u"])
        degree_v = int(operation["degree_v"])
        count_u = int(operation["control_point_count_u"])
        count_v = int(operation["control_point_count_v"])
        direction = str(operation["direction"]).lower()
        if direction not in ("u", "v"):
            raise ValueError("surface control-point insertion axis must be u or v")
        parameters = operation["parameter"]
        if len(parameters) != 2:
            raise ValueError("surface control-point insertion requires a u,v parameter")
        parameter_u = _finite(parameters[0], "surface U insertion parameter")
        parameter_v = _finite(parameters[1], "surface V insertion parameter")
        midpoint = bool(operation.get("midpoint", False))
        source = Rhino.Geometry.NurbsSurface.Create(
            3, True, degree_u + 1, degree_v + 1, count_u, count_v
        )
        if source is None:
            raise ValueError("could not allocate InsertControlPoint source surface")
        try:
            _set_surface_controls(
                source, operation["control_points"], count_u, count_v
            )
            _set_knots(source.KnotsU, operation["knots_u"], "surface U knot")
            _set_knots(source.KnotsV, operation["knots_v"], "surface V knot")
            if not source.IsValid:
                raise ValueError("InsertControlPoint source surface is invalid")
        except Exception:
            source.Dispose()
            raise
        point = source.PointAt(parameter_u, parameter_v)
        # Rhino's option names the orientation of the inserted row. A U row
        # therefore adds one control in the V parameter direction, and vice
        # versa; the protocol names the parameter axis whose count increases.
        command_direction = "V" if direction == "u" else "U"
        command = "_InsertControlPoint _Direction=_%s _Midpoint=_%s %s _Enter" % (
            command_direction,
            "Yes" if midpoint else "No",
            _command_point(_xyz(point)),
        )

        def insert_surface_control_point():
            object_id = System.Guid.Empty
            result = None
            try:
                document.Objects.UnselectAll()
                Rhino.RhinoApp.RunScript("_-CreaseSplitting _Disable", False)
                object_id = document.Objects.AddSurface(source)
                if object_id == System.Guid.Empty:
                    raise ValueError("could not add InsertControlPoint source surface")
                document.Objects.Select(object_id, True)
                if not Rhino.RhinoApp.RunScript(command, False):
                    history = Rhino.RhinoApp.CommandHistoryWindowText
                    raise ValueError(
                        "Rhino surface control-point insertion failed; history tail: %s"
                        % history[-3000:]
                    )
                rhino_object = document.Objects.FindId(object_id)
                if rhino_object is None:
                    raise ValueError("InsertControlPoint removed the surface")
                geometry = rhino_object.Geometry
                if isinstance(geometry, Rhino.Geometry.Brep):
                    if geometry.Faces.Count != 1:
                        raise ValueError(
                            "surface control-point insertion returned a polysurface"
                        )
                    result = geometry.Faces[0].UnderlyingSurface().ToNurbsSurface()
                else:
                    result = geometry.ToNurbsSurface()
                if result is None:
                    raise ValueError("InsertControlPoint returned no NURBS surface")
                expected_u = count_u + (1 if direction == "u" else 0)
                expected_v = count_v + (1 if direction == "v" else 0)
                if result.Points.CountU != expected_u or result.Points.CountV != expected_v:
                    raise ValueError(
                        "InsertControlPoint returned an unexpected surface control count"
                    )
                definition = _nurbs_surface_definition(result)
                definition["periodic_u"] = bool(result.IsPeriodic(0))
                definition["periodic_v"] = bool(result.IsPeriodic(1))
                return definition
            finally:
                if result is not None:
                    result.Dispose()
                existing = document.Objects.FindId(object_id)
                if existing is not None:
                    document.Objects.Delete(object_id, True)
                Rhino.RhinoApp.RunScript("_-CreaseSplitting _Enable", False)

        try:
            return _measure(iterations, insert_surface_control_point)
        finally:
            source.Dispose()

    if kind == "curve_remove_control_point_geometry":
        document = Rhino.RhinoDoc.ActiveDoc
        source = _nurbs_curve_from_definition(operation["curve"])
        control_point_index = int(operation["control_point_index"])

        def remove_control_point():
            object_id = System.Guid.Empty
            try:
                document.Objects.UnselectAll()
                object_id = document.Objects.AddCurve(source)
                if object_id == System.Guid.Empty:
                    raise ValueError("could not add RemoveControlPoint source curve")
                curve_object = document.Objects.FindId(object_id)
                curve_object.GripsOn = True
                grips = curve_object.GetGrips()
                if (
                    grips is None
                    or control_point_index < 0
                    or control_point_index >= len(grips)
                ):
                    raise ValueError("RemoveControlPoint control index is invalid")
                if int(grips[control_point_index].Select(True)) == 0:
                    raise ValueError("could not select RemoveControlPoint curve grip")
                if not Rhino.RhinoApp.RunScript("_Delete", False):
                    history = Rhino.RhinoApp.CommandHistoryWindowText
                    raise ValueError(
                        "Rhino curve control-point removal failed; history tail: %s"
                        % history[-3000:]
                    )
                rhino_object = document.Objects.FindId(object_id)
                if rhino_object is None:
                    raise ValueError("RemoveControlPoint removed the curve object")
                result = rhino_object.Geometry.ToNurbsCurve()
                if result is None:
                    raise ValueError("RemoveControlPoint returned no NURBS curve")
                try:
                    definition = _nurbs_curve_definition(result)
                    definition["closed"] = bool(result.IsClosed)
                    definition["periodic"] = bool(result.IsPeriodic)
                    return definition
                finally:
                    result.Dispose()
            finally:
                existing = document.Objects.FindId(object_id)
                if existing is not None:
                    existing.GripsOn = False
                    document.Objects.Delete(object_id, True)

        try:
            return _measure(iterations, remove_control_point)
        finally:
            source.Dispose()

    if kind == "surface_remove_control_point_geometry":
        document = Rhino.RhinoDoc.ActiveDoc
        degree_u = int(operation["degree_u"])
        degree_v = int(operation["degree_v"])
        count_u = int(operation["control_point_count_u"])
        count_v = int(operation["control_point_count_v"])
        direction = str(operation["direction"]).lower()
        if direction not in ("u", "v"):
            raise ValueError("surface control-point direction must be u or v")
        control_point_index = int(operation["control_point_index"])
        source = Rhino.Geometry.NurbsSurface.Create(
            3, True, degree_u + 1, degree_v + 1, count_u, count_v
        )
        if source is None:
            raise ValueError("could not allocate RemoveControlPoint source surface")
        try:
            _set_surface_controls(
                source, operation["control_points"], count_u, count_v
            )
            _set_knots(source.KnotsU, operation["knots_u"], "surface U knot")
            _set_knots(source.KnotsV, operation["knots_v"], "surface V knot")
            if not source.IsValid:
                raise ValueError("RemoveControlPoint source surface is invalid")
        except Exception:
            source.Dispose()
            raise

        def remove_surface_control_point():
            object_id = System.Guid.Empty
            result = None
            try:
                document.Objects.UnselectAll()
                Rhino.RhinoApp.RunScript("_-CreaseSplitting _Disable", False)
                object_id = document.Objects.AddSurface(source)
                if object_id == System.Guid.Empty:
                    raise ValueError("could not add RemoveControlPoint source surface")
                surface_object = document.Objects.FindId(object_id)
                surface_object.GripsOn = True
                grips = surface_object.GetGrips()
                if grips is None:
                    raise ValueError("could not enable surface grips")
                grip_count_u = count_u - degree_u if source.IsPeriodic(0) else count_u
                grip_count_v = count_v - degree_v if source.IsPeriodic(1) else count_v
                if direction == "u":
                    if control_point_index < 0 or control_point_index >= grip_count_u:
                        raise ValueError("surface U control-point index is invalid")
                    grip_indices = [
                        control_point_index * grip_count_v + v_index
                        for v_index in range(grip_count_v)
                    ]
                else:
                    if control_point_index < 0 or control_point_index >= grip_count_v:
                        raise ValueError("surface V control-point index is invalid")
                    grip_indices = [
                        u_index * grip_count_v + control_point_index
                        for u_index in range(grip_count_u)
                    ]
                if any(int(grips[index].Select(True)) == 0 for index in grip_indices):
                    raise ValueError("could not select RemoveControlPoint surface grips")
                if not Rhino.RhinoApp.RunScript("_Delete", False):
                    history = Rhino.RhinoApp.CommandHistoryWindowText
                    raise ValueError(
                        "Rhino surface control-point removal failed; history tail: %s"
                        % history[-3000:]
                    )
                rhino_object = document.Objects.FindId(object_id)
                if rhino_object is None:
                    raise ValueError("RemoveControlPoint removed the surface object")
                geometry = rhino_object.Geometry
                if isinstance(geometry, Rhino.Geometry.Brep):
                    if geometry.Faces.Count != 1:
                        raise ValueError("surface grip deletion returned a polysurface")
                    result = geometry.Faces[0].UnderlyingSurface().ToNurbsSurface()
                else:
                    result = geometry.ToNurbsSurface()
                if result is None:
                    raise ValueError("RemoveControlPoint returned no NURBS surface")
                definition = _nurbs_surface_definition(result)
                definition["periodic_u"] = bool(result.IsPeriodic(0))
                definition["periodic_v"] = bool(result.IsPeriodic(1))
                return definition
            finally:
                if result is not None:
                    result.Dispose()
                existing = document.Objects.FindId(object_id)
                if existing is not None:
                    existing.GripsOn = False
                    document.Objects.Delete(object_id, True)
                Rhino.RhinoApp.RunScript("_-CreaseSplitting _Enable", False)

        try:
            return _measure(iterations, remove_surface_control_point)
        finally:
            source.Dispose()

    if kind == "surface_make_uniform_geometry":
        degree_u = int(operation["degree_u"])
        degree_v = int(operation["degree_v"])
        count_u = int(operation["control_point_count_u"])
        count_v = int(operation["control_point_count_v"])
        direction_name = str(operation.get("direction", "both")).lower()
        directions = {"u": 0, "v": 1, "both": 2}
        if direction_name not in directions:
            raise ValueError("surface uniform direction must be u, v, or both")
        direction = directions[direction_name]
        command_name = operation.get("rhino_command")
        if command_name not in (None, "make_uniform", "make_uniform_uv"):
            raise ValueError(
                "surface uniform Rhino command must be make_uniform or make_uniform_uv"
            )
        if command_name == "make_uniform" and direction_name != "both":
            raise ValueError("Rhino MakeUniform always changes both surface directions")
        if command_name == "make_uniform_uv" and direction_name == "both":
            raise ValueError("Rhino MakeUniformUV command direction must be u or v")

        def make_uniform_surface():
            surface = Rhino.Geometry.NurbsSurface.Create(
                3, True, degree_u + 1, degree_v + 1, count_u, count_v
            )
            if surface is None:
                raise ValueError("could not allocate NURBS surface")
            try:
                _set_surface_controls(
                    surface, operation["control_points"], count_u, count_v
                )
                _set_knots(
                    surface.KnotsU, operation["knots_u"], "surface U knot"
                )
                _set_knots(
                    surface.KnotsV, operation["knots_v"], "surface V knot"
                )
                if not surface.IsValid:
                    raise ValueError("NURBS surface is invalid")
                if command_name is None:
                    if not surface.MakeUniform(direction):
                        raise ValueError("Rhino surface uniformization failed")
                    definition = _nurbs_surface_definition(surface)
                    definition["periodic_u"] = bool(surface.IsPeriodic(0))
                    definition["periodic_v"] = bool(surface.IsPeriodic(1))
                    return definition

                document = Rhino.RhinoDoc.ActiveDoc
                object_id = System.Guid.Empty
                result = None
                try:
                    document.Objects.UnselectAll()
                    object_id = document.Objects.AddSurface(surface)
                    if object_id == System.Guid.Empty:
                        raise ValueError("could not add surface to Rhino")
                    if not document.Objects.Select(object_id):
                        raise ValueError("could not select surface")
                    if command_name == "make_uniform":
                        command = "_-MakeUniform _All _Enter"
                    else:
                        command = "_-MakeUniformUV _Direction=_%s _All _Enter" % (
                            direction_name.capitalize()
                        )
                    Rhino.RhinoApp.RunScript(command, False)
                    rhino_object = document.Objects.FindId(object_id)
                    if rhino_object is None:
                        raise ValueError("surface uniform command removed the object")
                    geometry = rhino_object.Geometry
                    if isinstance(geometry, Rhino.Geometry.Brep):
                        if geometry.Faces.Count != 1:
                            history = Rhino.RhinoApp.CommandHistoryWindowText
                            raise ValueError(
                                "surface uniform command made a %d-face polysurface; "
                                "history tail: %s"
                                % (geometry.Faces.Count, history[-3000:])
                            )
                        result = geometry.Faces[0].UnderlyingSurface().ToNurbsSurface()
                    else:
                        result = geometry.ToNurbsSurface()
                    if result is None:
                        raise ValueError("surface uniform command returned no NURBS surface")
                    definition = _nurbs_surface_definition(result)
                    definition["periodic_u"] = bool(result.IsPeriodic(0))
                    definition["periodic_v"] = bool(result.IsPeriodic(1))
                    return definition
                finally:
                    if result is not None:
                        result.Dispose()
                    if object_id != System.Guid.Empty:
                        document.Objects.Delete(object_id, True)
            finally:
                surface.Dispose()

        return _measure(iterations, make_uniform_surface)

    if kind == "surface_insert_knot_geometry":
        degree_u = int(operation["degree_u"])
        degree_v = int(operation["degree_v"])
        count_u = int(operation["control_point_count_u"])
        count_v = int(operation["control_point_count_v"])
        direction = str(operation["direction"]).lower()
        if direction not in ("u", "v"):
            raise ValueError("surface knot direction must be u or v")
        parameter = _finite(operation["parameter"], "surface knot parameter")
        multiplicity = int(operation["multiplicity"])

        def insert_surface_knot():
            surface = Rhino.Geometry.NurbsSurface.Create(
                3, True, degree_u + 1, degree_v + 1, count_u, count_v
            )
            if surface is None:
                raise ValueError("could not allocate NURBS surface")
            try:
                _set_surface_controls(
                    surface, operation["control_points"], count_u, count_v
                )
                _set_knots(
                    surface.KnotsU, operation["knots_u"], "surface U knot"
                )
                _set_knots(
                    surface.KnotsV, operation["knots_v"], "surface V knot"
                )
                if not surface.IsValid:
                    raise ValueError("NURBS surface is invalid")
                knots = surface.KnotsU if direction == "u" else surface.KnotsV
                if not knots.InsertKnot(parameter, multiplicity):
                    raise ValueError("Rhino surface knot insertion failed")
                definition = _nurbs_surface_definition(surface)
                definition["periodic_u"] = bool(surface.IsPeriodic(0))
                definition["periodic_v"] = bool(surface.IsPeriodic(1))
                return definition
            finally:
                surface.Dispose()

        return _measure(iterations, insert_surface_knot)

    if kind == "surface_remove_knot_geometry":
        degree_u = int(operation["degree_u"])
        degree_v = int(operation["degree_v"])
        count_u = int(operation["control_point_count_u"])
        count_v = int(operation["control_point_count_v"])
        direction = str(operation["direction"]).lower()
        if direction not in ("u", "v"):
            raise ValueError("surface knot direction must be u or v")
        parameter = _finite(operation["parameter"], "surface knot parameter")

        def remove_surface_knot():
            surface = Rhino.Geometry.NurbsSurface.Create(
                3, True, degree_u + 1, degree_v + 1, count_u, count_v
            )
            if surface is None:
                raise ValueError("could not allocate NURBS surface")
            try:
                _set_surface_controls(
                    surface, operation["control_points"], count_u, count_v
                )
                _set_knots(
                    surface.KnotsU, operation["knots_u"], "surface U knot"
                )
                _set_knots(
                    surface.KnotsV, operation["knots_v"], "surface V knot"
                )
                if not surface.IsValid:
                    raise ValueError("NURBS surface is invalid")
                knots = surface.KnotsU if direction == "u" else surface.KnotsV
                degree = degree_u if direction == "u" else degree_v
                point_count = count_u if direction == "u" else count_v
                knot_index = min(
                    range(degree - 1, point_count),
                    key=lambda index: (
                        abs(float(knots[index]) - parameter),
                        -float(knots[index]),
                    ),
                )
                if not knots.RemoveKnots(knot_index, knot_index + 1):
                    raise ValueError("Rhino surface knot removal failed")
                definition = _nurbs_surface_definition(surface)
                definition["periodic_u"] = bool(surface.IsPeriodic(0))
                definition["periodic_v"] = bool(surface.IsPeriodic(1))
                return definition
            finally:
                surface.Dispose()

        return _measure(iterations, remove_surface_knot)

    if kind == "surface_remove_multi_knot_geometry":
        degree_u = int(operation["degree_u"])
        degree_v = int(operation["degree_v"])
        count_u = int(operation["control_point_count_u"])
        count_v = int(operation["control_point_count_v"])
        remove_fully = bool(operation.get("remove_fully_multiple_knots", False))
        max_kink_angle = _finite(
            operation.get("maximum_kink_angle_degrees", 1.0),
            "maximum kink angle",
        )
        source = Rhino.Geometry.NurbsSurface.Create(
            3, True, degree_u + 1, degree_v + 1, count_u, count_v
        )
        if source is None:
            raise ValueError("could not allocate RemoveMultiKnot source surface")
        try:
            _set_surface_controls(source, operation["control_points"], count_u, count_v)
            _set_knots(source.KnotsU, operation["knots_u"], "surface U knot")
            _set_knots(source.KnotsV, operation["knots_v"], "surface V knot")
            if not source.IsValid:
                raise ValueError("RemoveMultiKnot source surface is invalid")
        except Exception:
            source.Dispose()
            raise

        def remove_surface_multi_knots():
            document = Rhino.RhinoDoc.ActiveDoc
            object_id = System.Guid.Empty
            result = None
            try:
                document.Objects.UnselectAll()
                Rhino.RhinoApp.RunScript("_-CreaseSplitting _Disable", False)
                object_id = document.Objects.AddSurface(source)
                if object_id == System.Guid.Empty:
                    raise ValueError("could not add RemoveMultiKnot source surface")
                command = "_-RemoveMultiKnot _RemoveFullyMultipleKnots=_%s" % (
                    "Yes" if remove_fully else "No"
                )
                if remove_fully:
                    command += " _MaxKinkAngle=%.17g" % max_kink_angle
                command += " _SelID %s _Enter" % str(object_id)
                Rhino.RhinoApp.RunScript(command, False)
                rhino_object = document.Objects.FindId(object_id)
                if rhino_object is None:
                    raise ValueError("RemoveMultiKnot removed the source surface")
                geometry = rhino_object.Geometry
                if isinstance(geometry, Rhino.Geometry.Brep):
                    if geometry.Faces.Count != 1:
                        raise ValueError("RemoveMultiKnot returned a polysurface")
                    result = geometry.Faces[0].UnderlyingSurface().ToNurbsSurface()
                else:
                    result = geometry.ToNurbsSurface()
                if result is None:
                    raise ValueError("RemoveMultiKnot returned no NURBS surface")
                definition = _nurbs_surface_definition(result)
                definition["periodic_u"] = bool(result.IsPeriodic(0))
                definition["periodic_v"] = bool(result.IsPeriodic(1))
                return definition
            finally:
                if result is not None:
                    result.Dispose()
                if object_id != System.Guid.Empty:
                    document.Objects.Delete(object_id, True)
                Rhino.RhinoApp.RunScript("_-CreaseSplitting _Enable", False)

        try:
            return _measure(iterations, remove_surface_multi_knots)
        finally:
            source.Dispose()

    if kind == "curve_change_degree_geometry":
        source = _nurbs_curve_from_definition(operation["curve"])
        degree = int(operation["degree"])
        deformable = bool(operation.get("deformable", False))

        def change_curve_degree():
            document = Rhino.RhinoDoc.ActiveDoc
            object_id = System.Guid.Empty
            try:
                document.Objects.UnselectAll()
                object_id = document.Objects.AddCurve(source)
                if object_id == System.Guid.Empty:
                    raise ValueError("could not add ChangeDegree source curve")
                if not document.Objects.Select(object_id):
                    raise ValueError("could not select ChangeDegree source curve")
                command = "_-ChangeDegree _Deformable=_%s %d _Enter" % (
                    "Yes" if deformable else "No",
                    degree,
                )
                if not Rhino.RhinoApp.RunScript(command, False):
                    history = Rhino.RhinoApp.CommandHistoryWindowText
                    raise ValueError(
                        "Rhino curve ChangeDegree failed; history tail: %s"
                        % history[-3000:]
                    )
                rhino_object = document.Objects.FindId(object_id)
                if rhino_object is None:
                    raise ValueError("ChangeDegree removed the curve object")
                result = rhino_object.Geometry.ToNurbsCurve()
                if result is None:
                    raise ValueError("ChangeDegree returned no NURBS curve")
                try:
                    definition = _nurbs_curve_definition(result)
                    definition["closed"] = bool(result.IsClosed)
                    definition["periodic"] = bool(result.IsPeriodic)
                    return definition
                finally:
                    result.Dispose()
            finally:
                if object_id != System.Guid.Empty:
                    document.Objects.Delete(object_id, True)

        try:
            return _measure(iterations, change_curve_degree)
        finally:
            source.Dispose()

    if kind == "curve_make_periodic_geometry":
        source = _nurbs_curve_from_definition(operation["curve"])
        smooth = bool(operation.get("smooth", True))

        def make_curve_periodic():
            result = Rhino.Geometry.Curve.CreatePeriodicCurve(source, smooth)
            if result is None:
                raise ValueError("Rhino curve periodic conversion returned no result")
            try:
                nurbs = result.ToNurbsCurve()
                if nurbs is None:
                    raise ValueError("Rhino periodic curve has no NURBS representation")
                try:
                    definition = _nurbs_curve_definition(nurbs)
                    definition["closed"] = bool(nurbs.IsClosed)
                    definition["periodic"] = bool(nurbs.IsPeriodic)
                    return definition
                finally:
                    nurbs.Dispose()
            finally:
                result.Dispose()

        try:
            return _measure(iterations, make_curve_periodic)
        finally:
            source.Dispose()

    if kind == "curve_make_non_periodic_geometry":
        document = Rhino.RhinoDoc.ActiveDoc
        source = _nurbs_curve_from_definition(operation["curve"])

        def make_curve_non_periodic():
            duplicate = source.DuplicateCurve()
            if duplicate is None:
                raise ValueError("could not duplicate periodic curve")
            object_id = System.Guid.Empty
            result = None
            try:
                document.Objects.UnselectAll()
                object_id = document.Objects.AddCurve(duplicate)
                if object_id == System.Guid.Empty:
                    raise ValueError("could not add periodic curve to Rhino")
                if not document.Objects.Select(object_id):
                    raise ValueError("could not select periodic curve")
                Rhino.RhinoApp.RunScript("_-MakeNonPeriodic _Enter", False)
                rhino_object = document.Objects.FindId(object_id)
                if rhino_object is None:
                    raise ValueError("MakeNonPeriodic removed the curve object")
                result = rhino_object.Geometry.ToNurbsCurve()
                if result is None or result.IsPeriodic:
                    history = Rhino.RhinoApp.CommandHistoryWindowText
                    raise ValueError(
                        "MakeNonPeriodic did not produce a non-periodic curve; "
                        "history tail: %s" % history[-3000:]
                    )
                definition = _nurbs_curve_definition(result)
                definition["closed"] = bool(result.IsClosed)
                definition["periodic"] = bool(result.IsPeriodic)
                return definition
            finally:
                if result is not None:
                    result.Dispose()
                if object_id != System.Guid.Empty:
                    document.Objects.Delete(object_id, True)
                duplicate.Dispose()

        try:
            return _measure(iterations, make_curve_non_periodic)
        finally:
            source.Dispose()

    if kind == "surface_change_degree_geometry":
        degree_u = int(operation["degree_u"])
        degree_v = int(operation["degree_v"])
        count_u = int(operation["control_point_count_u"])
        count_v = int(operation["control_point_count_v"])
        desired_u = int(operation["desired_degree_u"])
        desired_v = int(operation["desired_degree_v"])
        deformable = bool(operation.get("deformable", False))
        source = Rhino.Geometry.NurbsSurface.Create(
            3, True, degree_u + 1, degree_v + 1, count_u, count_v
        )
        if source is None:
            raise ValueError("could not allocate ChangeDegree source surface")
        try:
            _set_surface_controls(
                source, operation["control_points"], count_u, count_v
            )
            _set_knots(source.KnotsU, operation["knots_u"], "surface U knot")
            _set_knots(source.KnotsV, operation["knots_v"], "surface V knot")
            if not source.IsValid:
                raise ValueError("ChangeDegree source surface is invalid")
        except Exception:
            source.Dispose()
            raise

        def change_surface_degree():
            document = Rhino.RhinoDoc.ActiveDoc
            object_id = System.Guid.Empty
            result = None
            try:
                document.Objects.UnselectAll()
                object_id = document.Objects.AddSurface(source)
                if object_id == System.Guid.Empty:
                    raise ValueError("could not add ChangeDegree source surface")
                if not document.Objects.Select(object_id):
                    raise ValueError("could not select ChangeDegree source surface")
                command = "_-ChangeDegree _Deformable=_%s %d %d _Enter" % (
                    "Yes" if deformable else "No",
                    desired_u,
                    desired_v,
                )
                if not Rhino.RhinoApp.RunScript(command, False):
                    history = Rhino.RhinoApp.CommandHistoryWindowText
                    raise ValueError(
                        "Rhino surface ChangeDegree failed; history tail: %s"
                        % history[-3000:]
                    )
                rhino_object = document.Objects.FindId(object_id)
                if rhino_object is None:
                    raise ValueError("ChangeDegree removed the surface object")
                geometry = rhino_object.Geometry
                if isinstance(geometry, Rhino.Geometry.Brep):
                    if geometry.Faces.Count != 1:
                        raise ValueError("ChangeDegree returned a polysurface")
                    result = geometry.Faces[0].UnderlyingSurface().ToNurbsSurface()
                else:
                    result = geometry.ToNurbsSurface()
                if result is None:
                    raise ValueError("ChangeDegree returned no NURBS surface")
                definition = _nurbs_surface_definition(result)
                definition["periodic_u"] = bool(result.IsPeriodic(0))
                definition["periodic_v"] = bool(result.IsPeriodic(1))
                return definition
            finally:
                if result is not None:
                    result.Dispose()
                if object_id != System.Guid.Empty:
                    document.Objects.Delete(object_id, True)

        try:
            return _measure(iterations, change_surface_degree)
        finally:
            source.Dispose()

    if kind == "surface_make_periodic_geometry":
        degree_u = int(operation["degree_u"])
        degree_v = int(operation["degree_v"])
        count_u = int(operation["control_point_count_u"])
        count_v = int(operation["control_point_count_v"])
        direction_name = str(operation["direction"]).lower()
        directions = {"u": 0, "v": 1}
        command_name = operation.get("rhino_command")
        if command_name not in (None, "make_periodic"):
            raise ValueError("surface periodic Rhino command must be make_periodic")
        if command_name is None and direction_name not in directions:
            raise ValueError("surface periodic API direction must be u or v")
        if command_name is not None and direction_name not in ("u", "v", "both"):
            raise ValueError("surface periodic command direction must be u, v, or both")
        smooth = bool(operation.get("smooth", True))
        source = Rhino.Geometry.NurbsSurface.Create(
            3, True, degree_u + 1, degree_v + 1, count_u, count_v
        )
        if source is None:
            raise ValueError("could not allocate closed NURBS surface")
        try:
            _set_surface_controls(
                source, operation["control_points"], count_u, count_v
            )
            _set_knots(source.KnotsU, operation["knots_u"], "surface U knot")
            _set_knots(source.KnotsV, operation["knots_v"], "surface V knot")
            if not source.IsValid:
                raise ValueError("closed NURBS surface is invalid")
        except Exception:
            source.Dispose()
            raise

        def make_surface_periodic():
            if command_name is None:
                result = Rhino.Geometry.Surface.CreatePeriodicSurface(
                    source, directions[direction_name], smooth
                )
                if result is None:
                    raise ValueError(
                        "Rhino surface periodic conversion returned no result"
                    )
                try:
                    nurbs = result.ToNurbsSurface()
                    if nurbs is None:
                        raise ValueError(
                            "Rhino periodic surface has no NURBS representation"
                        )
                    try:
                        definition = _nurbs_surface_definition(nurbs)
                        definition["periodic_u"] = bool(nurbs.IsPeriodic(0))
                        definition["periodic_v"] = bool(nurbs.IsPeriodic(1))
                        return definition
                    finally:
                        nurbs.Dispose()
                finally:
                    result.Dispose()

            document = Rhino.RhinoDoc.ActiveDoc
            object_id = System.Guid.Empty
            result = None
            try:
                document.Objects.UnselectAll()
                object_id = document.Objects.AddSurface(source)
                if object_id == System.Guid.Empty:
                    raise ValueError("could not add closed surface to Rhino")
                command = (
                    "_-MakePeriodic _Smooth=_%s '_-SelID %s _Enter "
                    "_DeleteInput=_Yes _Enter"
                ) % (
                    "Yes" if smooth else "No",
                    object_id,
                )
                Rhino.RhinoApp.RunScript(command, False)
                rhino_object = document.Objects.FindId(object_id)
                if rhino_object is None:
                    raise ValueError("MakePeriodic removed the surface object")
                geometry = rhino_object.Geometry
                if isinstance(geometry, Rhino.Geometry.Brep):
                    if geometry.Faces.Count != 1:
                        history = Rhino.RhinoApp.CommandHistoryWindowText
                        raise ValueError(
                            "MakePeriodic made a %d-face polysurface; history tail: %s"
                            % (geometry.Faces.Count, history[-3000:])
                        )
                    result = geometry.Faces[0].UnderlyingSurface().ToNurbsSurface()
                else:
                    result = geometry.ToNurbsSurface()
                if result is None:
                    history = Rhino.RhinoApp.CommandHistoryWindowText
                    raise ValueError(
                        "MakePeriodic returned no NURBS surface; history tail: %s"
                        % history[-3000:]
                    )
                definition = _nurbs_surface_definition(result)
                definition["periodic_u"] = bool(result.IsPeriodic(0))
                definition["periodic_v"] = bool(result.IsPeriodic(1))
                return definition
            finally:
                if result is not None:
                    result.Dispose()
                if object_id != System.Guid.Empty:
                    document.Objects.Delete(object_id, True)

        try:
            return _measure(iterations, make_surface_periodic)
        finally:
            source.Dispose()

    if kind == "surface_make_non_periodic_geometry":
        document = Rhino.RhinoDoc.ActiveDoc
        degree_u = int(operation["degree_u"])
        degree_v = int(operation["degree_v"])
        count_u = int(operation["control_point_count_u"])
        count_v = int(operation["control_point_count_v"])
        source = Rhino.Geometry.NurbsSurface.Create(
            3, True, degree_u + 1, degree_v + 1, count_u, count_v
        )
        if source is None:
            raise ValueError("could not allocate periodic NURBS surface")
        try:
            _set_surface_controls(
                source, operation["control_points"], count_u, count_v
            )
            _set_knots(source.KnotsU, operation["knots_u"], "surface U knot")
            _set_knots(source.KnotsV, operation["knots_v"], "surface V knot")
            if not source.IsValid:
                raise ValueError("periodic NURBS surface is invalid")
        except Exception:
            source.Dispose()
            raise

        def make_surface_non_periodic():
            duplicate = source.Duplicate()
            if duplicate is None:
                raise ValueError("could not duplicate periodic surface")
            object_id = System.Guid.Empty
            result = None
            try:
                document.Objects.UnselectAll()
                object_id = document.Objects.AddSurface(duplicate)
                if object_id == System.Guid.Empty:
                    raise ValueError("could not add periodic surface to Rhino")
                if not document.Objects.Select(object_id):
                    raise ValueError("could not select periodic surface")
                Rhino.RhinoApp.RunScript("_-MakeNonPeriodic _Enter", False)
                rhino_object = document.Objects.FindId(object_id)
                if rhino_object is None:
                    raise ValueError("MakeNonPeriodic removed the surface object")
                geometry = rhino_object.Geometry
                if isinstance(geometry, Rhino.Geometry.Brep):
                    if geometry.Faces.Count != 1:
                        raise ValueError("MakeNonPeriodic surface became a polysurface")
                    result = geometry.Faces[0].UnderlyingSurface().ToNurbsSurface()
                else:
                    result = geometry.ToNurbsSurface()
                if result is None or result.IsPeriodic(0) or result.IsPeriodic(1):
                    history = Rhino.RhinoApp.CommandHistoryWindowText
                    raise ValueError(
                        "MakeNonPeriodic left a periodic surface direction; "
                        "history tail: %s" % history[-3000:]
                    )
                definition = _nurbs_surface_definition(result)
                definition["periodic_u"] = bool(result.IsPeriodic(0))
                definition["periodic_v"] = bool(result.IsPeriodic(1))
                return definition
            finally:
                if result is not None:
                    result.Dispose()
                if object_id != System.Guid.Empty:
                    document.Objects.Delete(object_id, True)
                duplicate.Dispose()

        try:
            return _measure(iterations, make_surface_non_periodic)
        finally:
            source.Dispose()

    if kind == "conic":
        document = Rhino.RhinoDoc.ActiveDoc
        start = _point(operation["start"])
        apex = _point(operation["apex"])
        end = _point(operation["end"])
        if bool(operation.get("apex_first", False)):
            command = "_-Conic %s _Apex %s %s" % (
                _command_point(_xyz(start)),
                _command_point(_xyz(apex)),
                _command_point(_xyz(end)),
            )
        else:
            command = "_-Conic %s %s %s" % (
                _command_point(_xyz(start)),
                _command_point(_xyz(end)),
                _command_point(_xyz(apex)),
            )
        definition = operation["definition"]
        mode = str(definition["mode"])
        if mode == "rho":
            rho = _finite(definition["value"], "conic rho")
            if not 0.0 < rho < 1.0:
                raise ValueError("conic rho must be between zero and one")
            command += " %.17g" % rho
        elif mode == "through_point":
            command += " " + _command_point(
                _xyz(_point(definition["point"]))
            )
        else:
            raise ValueError("unknown conic definition mode: %s" % mode)

        def create_conic():
            before = set(obj.Id for obj in document.Objects)
            document.Objects.UnselectAll()
            succeeded = Rhino.RhinoApp.RunScript(command, False)
            created = [obj for obj in document.Objects if obj.Id not in before]
            curve = None
            try:
                if len(created) != 1:
                    history = Rhino.RhinoApp.CommandHistoryWindowText
                    raise ValueError(
                        "conic macro %r returned %r and created %d objects; "
                        "history tail: %s"
                        % (command, succeeded, len(created), history[-3000:])
                    )
                geometry = created[0].Geometry
                if isinstance(geometry, Rhino.Geometry.Curve):
                    curve = geometry.DuplicateCurve()
                if curve is None:
                    raise ValueError("conic did not create curve geometry")
                return _nurbs_curve_definition(curve)
            finally:
                if curve is not None:
                    curve.Dispose()
                for obj in created:
                    document.Objects.Delete(obj.Id, True)

        return _measure(iterations, create_conic)

    if kind == "hyperbola":
        document = Rhino.RhinoDoc.ActiveDoc
        plane = Rhino.Geometry.Plane(
            _point(operation["origin"]),
            _vector(operation["x_axis"]),
            _vector(operation["y_axis"]),
        )
        transform = Rhino.Geometry.Transform.PlaneToPlane(
            Rhino.Geometry.Plane.WorldXY, plane
        )
        semi_transverse_axis = _finite(
            operation["semi_transverse_axis"], "hyperbola A coefficient"
        )
        semi_conjugate_axis = _finite(
            operation["semi_conjugate_axis"], "hyperbola B coefficient"
        )
        axial_extent = _finite(
            operation["axial_extent"], "hyperbola axial extent"
        )
        if (
            not semi_transverse_axis > 0.0
            or not semi_conjugate_axis > 0.0
            or not axial_extent > semi_transverse_axis
        ):
            raise ValueError("hyperbola dimensions are degenerate")
        both_branches = bool(operation["both_branches"])
        command = (
            "_-Hyperbola _FromCoefficient 0,0,0 1,0,0 "
            "_A=%.17g _B=%.17g _BothBranches=_%s _MarkFoci=_No "
            "_ShowAsymptotes=_No %.17g,-1,0"
            % (
                semi_transverse_axis,
                semi_conjugate_axis,
                "Yes" if both_branches else "No",
                axial_extent,
            )
        )

        def create_hyperbola():
            before = set(obj.Id for obj in document.Objects)
            document.Objects.UnselectAll()
            succeeded = Rhino.RhinoApp.RunScript(command, False)
            created = [obj for obj in document.Objects if obj.Id not in before]
            curves = []
            try:
                expected_count = 2 if both_branches else 1
                if len(created) != expected_count:
                    history = Rhino.RhinoApp.CommandHistoryWindowText
                    raise ValueError(
                        "hyperbola macro %r returned %r and created %d objects; "
                        "expected %d; history tail: %s"
                        % (
                            command,
                            succeeded,
                            len(created),
                            expected_count,
                            history[-3000:],
                        )
                    )
                for rhino_object in created:
                    geometry = rhino_object.Geometry
                    curve = None
                    if isinstance(geometry, Rhino.Geometry.Curve):
                        curve = geometry.DuplicateCurve()
                    if curve is None:
                        raise ValueError("hyperbola did not create curve geometry")
                    if not curve.Reverse():
                        curve.Dispose()
                        raise ValueError("could not normalize hyperbola direction")
                    curve.Domain = Rhino.Geometry.Interval(0.0, 1.0)
                    if not curve.Transform(transform):
                        curve.Dispose()
                        raise ValueError("could not orient hyperbola")
                    curves.append(curve)
                return {
                    "curves": [
                        _nurbs_curve_definition(curve) for curve in curves
                    ]
                }
            finally:
                for curve in curves:
                    curve.Dispose()
                for obj in created:
                    document.Objects.Delete(obj.Id, True)

        return _measure(iterations, create_hyperbola)

    if kind == "paraboloid":
        document = Rhino.RhinoDoc.ActiveDoc
        plane = Rhino.Geometry.Plane(
            _point(operation["origin"]),
            _vector(operation["x_axis"]),
            _vector(operation["y_axis"]),
        )
        transform = Rhino.Geometry.Transform.PlaneToPlane(
            Rhino.Geometry.Plane.WorldXY, plane
        )
        radius = _finite(operation["radius"], "paraboloid radius")
        height = _finite(operation["height"], "paraboloid height")
        if not radius > 0.0 or not height > 0.0:
            raise ValueError("paraboloid dimensions must be positive")
        focal_distance = 0.25 * radius * (radius / height)
        if math.isnan(focal_distance) or math.isinf(focal_distance):
            raise ValueError("paraboloid focal distance must be finite")
        solid = bool(operation["solid"])
        command = (
            "_-Paraboloid _Vertex _MarkFocus=_No _Solid=_%s "
            "0,0,0 0,0,%.17g %.17g,0,0"
            % ("Yes" if solid else "No", focal_distance, radius)
        )

        def create_paraboloid():
            before = set(obj.Id for obj in document.Objects)
            document.Objects.UnselectAll()
            succeeded = Rhino.RhinoApp.RunScript(command, False)
            created = [obj for obj in document.Objects if obj.Id not in before]
            brep = None
            try:
                if len(created) != 1:
                    history = Rhino.RhinoApp.CommandHistoryWindowText
                    raise ValueError(
                        "paraboloid macro %r returned %r and created %d objects; "
                        "history tail: %s"
                        % (command, succeeded, len(created), history[-3000:])
                    )
                geometry = created[0].Geometry
                if isinstance(geometry, Rhino.Geometry.Brep):
                    brep = geometry.DuplicateBrep()
                elif hasattr(geometry, "ToBrep"):
                    brep = geometry.ToBrep()
                if brep is None:
                    raise ValueError(
                        "paraboloid did not create B-rep-compatible geometry"
                    )
                if not brep.Transform(transform):
                    raise ValueError("could not orient paraboloid")
                expected_faces = 2 if solid else 1
                if brep.Faces.Count != expected_faces:
                    raise ValueError(
                        "paraboloid macro %r created %d faces, expected %d"
                        % (command, brep.Faces.Count, expected_faces)
                    )
                return {
                    "brep": _mesh_to_nurb_brep_value(brep),
                    "surfaces": [
                        _nurbs_surface_definition(face.UnderlyingSurface())
                        for face in brep.Faces
                    ],
                }
            finally:
                if brep is not None:
                    brep.Dispose()
                for obj in created:
                    document.Objects.Delete(obj.Id, True)

        return _measure(iterations, create_paraboloid)

    if kind == "pyramid" or kind == "truncated_pyramid":
        document = Rhino.RhinoDoc.ActiveDoc
        plane = Rhino.Geometry.Plane(
            _point(operation["origin"]),
            _vector(operation["x_axis"]),
            _vector(operation["y_axis"]),
        )
        transform = Rhino.Geometry.Transform.PlaneToPlane(
            Rhino.Geometry.Plane.WorldXY, plane
        )
        command_name = "Pyramid" if kind == "pyramid" else "TruncatedPyramid"
        side_count = int(operation["side_count"])
        base_radius = _finite(
            operation["radius"] if kind == "pyramid" else operation["base_radius"],
            "pyramid base radius",
        )
        height = _finite(operation["height"], "pyramid height")
        solid = bool(operation["solid"])
        if kind == "pyramid":
            command = (
                "_-%s _NumSides=%d _DirectionConstraint=_Vertical _Solid=_%s "
                "0,0,0 %.17g,0,0 0,0,%.17g"
                % (
                    command_name,
                    side_count,
                    "Yes" if solid else "No",
                    base_radius,
                    height,
                )
            )
        else:
            top_radius = _finite(operation["top_radius"], "pyramid top radius")
            command = (
                "_-%s _NumSides=%d _DirectionConstraint=_Vertical _Solid=_%s "
                "0,0,0 %.17g,0,0 0,0,%.17g %.17g"
                % (
                    command_name,
                    side_count,
                    "Yes" if solid else "No",
                    base_radius,
                    height,
                    top_radius,
                )
            )

        def create_pyramid():
            before = set(obj.Id for obj in document.Objects)
            document.Objects.UnselectAll()
            succeeded = Rhino.RhinoApp.RunScript(command, False)
            created = [obj for obj in document.Objects if obj.Id not in before]
            brep = None
            try:
                if len(created) != 1:
                    history = Rhino.RhinoApp.CommandHistoryWindowText
                    raise ValueError(
                        "pyramid macro %r returned %r and created %d objects; history tail: %s"
                        % (command, succeeded, len(created), history[-3000:])
                    )
                geometry = created[0].Geometry
                if isinstance(geometry, Rhino.Geometry.Brep):
                    brep = geometry.DuplicateBrep()
                elif hasattr(geometry, "ToBrep"):
                    brep = geometry.ToBrep()
                if brep is None:
                    raise ValueError("pyramid did not create B-rep-compatible geometry")
                if not brep.Transform(transform):
                    raise ValueError("could not orient pyramid")
                expected_faces = side_count
                if solid:
                    expected_faces += 2 if kind == "truncated_pyramid" else 1
                if brep.Faces.Count != expected_faces:
                    raise ValueError(
                        "pyramid macro %r created %d faces, expected %d"
                        % (command, brep.Faces.Count, expected_faces)
                    )
                return {
                    "brep": _mesh_to_nurb_brep_value(brep),
                    "surfaces": [
                        _nurbs_surface_definition(face.UnderlyingSurface())
                        for face in brep.Faces
                    ],
                }
            finally:
                if brep is not None:
                    brep.Dispose()
                for obj in created:
                    document.Objects.Delete(obj.Id, True)

        return _measure(iterations, create_pyramid)

    if kind == "truncated_cone":
        document = Rhino.RhinoDoc.ActiveDoc
        plane = Rhino.Geometry.Plane(
            _point(operation["origin"]),
            _vector(operation["x_axis"]),
            _vector(operation["y_axis"]),
        )
        base_radius = _finite(
            operation["base_radius"], "truncated-cone base radius"
        )
        end_radius = _finite(
            operation["end_radius"], "truncated-cone end radius"
        )
        height = _finite(operation["height"], "truncated-cone height")
        solid = bool(operation["solid"])
        command = (
            "_-TruncatedCone _DirectionConstraint=_Vertical _Solid=_%s "
            "0,0,0 %.17g %.17g %.17g"
            % ("Yes" if solid else "No", base_radius, height, end_radius)
        )
        transform = Rhino.Geometry.Transform.PlaneToPlane(
            Rhino.Geometry.Plane.WorldXY, plane
        )

        def create_truncated_cone():
            before = set(obj.Id for obj in document.Objects)
            document.Objects.UnselectAll()
            succeeded = Rhino.RhinoApp.RunScript(command, False)
            created = [obj for obj in document.Objects if obj.Id not in before]
            brep = None
            try:
                if len(created) != 1:
                    history = Rhino.RhinoApp.CommandHistoryWindowText
                    raise ValueError(
                        "TruncatedCone macro %r returned %r and created %d objects; history tail: %s"
                        % (command, succeeded, len(created), history[-2000:])
                    )
                geometry = created[0].Geometry
                if isinstance(geometry, Rhino.Geometry.Brep):
                    brep = geometry.DuplicateBrep()
                elif hasattr(geometry, "ToBrep"):
                    brep = geometry.ToBrep()
                if brep is None:
                    raise ValueError("truncated cone did not create B-rep-compatible geometry")
                if not brep.Transform(transform):
                    raise ValueError("could not orient truncated cone")
                expected_faces = 3 if solid else 1
                if brep.Faces.Count != expected_faces:
                    raise ValueError(
                        "TruncatedCone macro %r created %d faces, expected %d"
                        % (command, brep.Faces.Count, expected_faces)
                    )
                return {
                    "brep": _mesh_to_nurb_brep_value(brep) if solid else None,
                    "wall": _nurbs_surface_definition(
                        brep.Faces[0].UnderlyingSurface()
                    ),
                }
            finally:
                if brep is not None:
                    brep.Dispose()
                for obj in created:
                    document.Objects.Delete(obj.Id, True)

        return _measure(iterations, create_truncated_cone)

    if kind == "tube":
        document = Rhino.RhinoDoc.ActiveDoc
        plane = Rhino.Geometry.Plane(
            _point(operation["origin"]),
            _vector(operation["x_axis"]),
            _vector(operation["y_axis"]),
        )
        inner_radius = _finite(operation["inner_radius"], "tube inner radius")
        outer_radius = _finite(operation["outer_radius"], "tube outer radius")
        height = _finite(operation["height"], "tube height")
        command = (
            "_-Tube _DirectionConstraint=_Vertical 0,0,0 %.17g %.17g "
            "_BothSides=_No %.17g"
            % (outer_radius, inner_radius, height)
        )
        transform = Rhino.Geometry.Transform.PlaneToPlane(
            Rhino.Geometry.Plane.WorldXY, plane
        )

        def create_tube():
            before = set(obj.Id for obj in document.Objects)
            document.Objects.UnselectAll()
            succeeded = Rhino.RhinoApp.RunScript(command, False)
            created = [obj for obj in document.Objects if obj.Id not in before]
            brep = None
            try:
                if len(created) != 1:
                    history = Rhino.RhinoApp.CommandHistoryWindowText
                    raise ValueError(
                        "Tube macro %r returned %r and created %d objects; history tail: %s"
                        % (command, succeeded, len(created), history[-2000:])
                    )
                geometry = created[0].Geometry
                if isinstance(geometry, Rhino.Geometry.Brep):
                    brep = geometry.DuplicateBrep()
                elif hasattr(geometry, "ToBrep"):
                    brep = geometry.ToBrep()
                if brep is None:
                    raise ValueError("tube did not create B-rep-compatible geometry")
                if not brep.Transform(transform):
                    raise ValueError("could not orient tube")
                if brep.Faces.Count != 4:
                    raise ValueError(
                        "Tube macro %r created %d faces, expected 4"
                        % (command, brep.Faces.Count)
                    )
                return {
                    "brep": _mesh_to_nurb_brep_value(brep),
                    "surfaces": [
                        _nurbs_surface_definition(face.UnderlyingSurface())
                        for face in brep.Faces
                    ],
                }
            finally:
                if brep is not None:
                    brep.Dispose()
                for obj in created:
                    document.Objects.Delete(obj.Id, True)

        return _measure(iterations, create_tube)

    if kind == "mesh_quad_sphere" or kind == "mesh_ico_sphere":
        plane = Rhino.Geometry.Plane(
            _point(operation["origin"]),
            _vector(operation["x_axis"]),
            _vector(operation["y_axis"]),
        )
        sphere = Rhino.Geometry.Sphere(
            plane,
            _finite(operation["radius"], "subdivision mesh-sphere radius"),
        )
        subdivisions = int(operation["subdivisions"])

        def create_subdivision_mesh_sphere():
            if kind == "mesh_quad_sphere":
                mesh = Rhino.Geometry.Mesh.CreateQuadSphere(sphere, subdivisions)
            else:
                mesh = Rhino.Geometry.Mesh.CreateIcoSphere(sphere, subdivisions)
            if mesh is None:
                raise ValueError("could not create subdivision mesh sphere")
            try:
                return _polygon_mesh_value(mesh)
            finally:
                mesh.Dispose()

        return _measure(iterations, create_subdivision_mesh_sphere)

    if kind == "mesh_torus":
        plane = Rhino.Geometry.Plane(
            _point(operation["origin"]),
            _vector(operation["x_axis"]),
            _vector(operation["y_axis"]),
        )
        major_radius = _finite(
            operation["major_radius"], "mesh-torus major radius"
        )
        minor_radius = _finite(
            operation["minor_radius"], "mesh-torus minor radius"
        )
        torus = Rhino.Geometry.Torus(plane, major_radius, minor_radius)
        vertical = int(operation["vertical"])
        around = int(operation["around"])

        def create_mesh_torus():
            mesh = Rhino.Geometry.Mesh.CreateFromTorus(
                torus,
                vertical,
                around,
            )
            if mesh is None:
                raise ValueError("could not create mesh torus")
            try:
                return _polygon_mesh_value(mesh)
            finally:
                mesh.Dispose()

        return _measure(iterations, create_mesh_torus)

    if kind == "nurbs_surface_mesh":
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
            surface.Dispose()
            raise ValueError("NURBS surface is invalid")
        density = _finite(operation.get("density", 0.5), "mesh density")
        if density < 0.0 or density > 1.0:
            surface.Dispose()
            raise ValueError("mesh density must lie in [0, 1]")
        parameters = Rhino.Geometry.MeshingParameters(density)
        parameters.JaggedSeams = bool(operation.get("jagged_seams", False))
        parameters.SimplePlanes = bool(operation.get("simple_planes", False))

        def mesh_surface():
            mesh = Rhino.Geometry.Mesh.CreateFromSurface(surface, parameters)
            if mesh is None:
                raise ValueError("could not mesh NURBS surface")
            try:
                return _canonical_polygon_mesh_face_value(mesh)
            finally:
                mesh.Dispose()

        try:
            return _measure(iterations, mesh_surface)
        finally:
            parameters.Dispose()
            surface.Dispose()

    if kind == "nurbs_surface_extract_points":
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
            surface.Dispose()
            raise ValueError("NURBS surface is invalid")
        document = Rhino.RhinoDoc.ActiveDoc
        object_id = document.Objects.AddSurface(surface)
        if object_id == System.Guid.Empty:
            surface.Dispose()
            raise ValueError("could not add NURBS surface grip probe")
        surface_object = document.Objects.FindId(object_id)
        try:
            surface_object.GripsOn = True
            grips = surface_object.GetGrips()
            if grips is None:
                raise ValueError("could not enable NURBS surface grips")
            return _measure(
                iterations,
                lambda: [_xyz(grip.CurrentLocation) for grip in grips],
            )
        finally:
            surface_object.GripsOn = False
            document.Objects.Delete(object_id, True)
            surface.Dispose()

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


@contextmanager
def _document_tolerance(document, tolerance):
    """Run command macros with the same tolerance as the native document."""
    if document is None:
        raise ValueError("oracle requires an active Rhino document")
    properties = (
        ("ModelAbsoluteTolerance", "absolute"),
        ("ModelRelativeTolerance", "relative"),
        ("ModelAngleToleranceRadians", "angular"),
    )
    previous = [(name, getattr(document, name)) for name, _key in properties]
    try:
        for name, key in properties:
            setattr(document, name, tolerance[key])
            if getattr(document, name) != tolerance[key]:
                raise ValueError("Rhino did not accept the requested %s" % name)
        yield
    finally:
        for name, value in previous:
            setattr(document, name, value)


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
        with _document_tolerance(Rhino.RhinoDoc.ActiveDoc, tolerance):
            for operation in operations:
                _record_progress(
                    "operation %s: start"
                    % operation.get("id", operation.get("op", "unknown"))
                )
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
        response["error"] = "%s at %s: %s" % (
            type(error).__name__,
            LAST_PROGRESS_STAGE,
            error,
        )
    return response


def _main():
    _record_progress("worker: started")
    job_directory = os.path.dirname(os.path.abspath(__file__))
    request_path = os.path.join(job_directory, "request.json")
    response_path = os.path.join(job_directory, "response.json")
    temporary_path = response_path + ".tmp"
    request = {}
    try:
        with open(request_path, "r") as stream:
            request = json.load(stream)
        _record_progress("worker: request loaded")
        response = _response(request)
        _record_progress("worker: response computed")
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
    _record_progress("worker: response published")
    host_options = {}
    if isinstance(request, dict):
        host_options = request.get("_host") or {}
    if host_options.get("exit_rhino_when_complete"):
        Rhino.RhinoApp.Exit(False)


if __name__ == "__main__":
    _main()
