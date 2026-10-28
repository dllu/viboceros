"""One-rail sweep orchestration; Rhino services are supplied by the worker.

Keep this companion importable without Rhino and compatible with IronPython.
Ownership and sampling policy are shared by SDK and actual command paths.
"""


def validate(operation):
    legacy = operation.get("legacy_refit", False)
    if type(legacy) is not bool:
        raise ValueError("legacy_refit must be a boolean")
    if legacy and (operation.get("command", False) or
                   operation.get("refit_tolerance") is None or
                   any(key in operation for key in ["start_point", "end_point", "roadlike_axis"]) or
                   operation.get("roadlike", False)):
        raise ValueError("legacy_refit supports only freeform SDK Refit without point ends")
    fit = operation.get("refit_tolerance")
    if fit is not None:
        import math
        valid = not isinstance(fit, bool) and isinstance(fit, (int, float))
        try:
            valid = valid and not math.isnan(fit) and not math.isinf(fit) and fit >= 0
        except (TypeError, ValueError, OverflowError):
            valid = False
        if not valid:
            raise ValueError("Sweep1 refit tolerance must be finite and nonnegative")
    count = operation.get("rebuild_point_count", 0)
    if fit is not None and count:
        raise ValueError("Sweep1 Rebuild and Refit are mutually exclusive")
    if type(count) is not int or not 0 <= count <= 512:
        raise ValueError("Sweep1 rebuild point count must be an integer from 0 to 512")
    if operation.get("command", False) and 0 < count < 3:
        raise ValueError("Sweep1 command rebuilding requires at least three points")


def run(host, operation, iterations, tolerance):
    validate(operation)
    Rhino, System = host["Rhino"], host["System"]
    owned, result = [], []
    try:
        rail = host["_join_close_input"](operation["rail"])
        owned.append(rail)
        for definition in operation["sections"]:
            owned.append(host["_join_close_input"](definition))

        if operation.get("command", False):
            def macro(ids):
                Rhino.RhinoDoc.ActiveDoc.Objects.UnselectAll()
                return host["_sweep1_macro"](operation, ids, any(curve.IsClosed for curve in owned[1:]))
            value, elapsed = host["_curve_surface_command"](owned, macro, iterations, "sweep",
                lambda obj: host["_sweep1_record"](obj.Geometry, operation), verify_script=True)
            records = [face for output in value["outputs"] for face in output]
            return records, 0

        roadlike_axis = host["_sweep1_roadlike_axis"](operation, rail, tolerance)

        def build():
            for brep in result:
                brep.Dispose()
            del result[:]
            if operation.get("legacy_refit", False):
                sweep = Rhino.Geometry.SweepOneRail()
                sweep.SweepTolerance = tolerance["absolute"]
                sweep.AngleToleranceRadians = tolerance["angular"]
                sweep.ClosedSweep = operation.get("closed", False)
                sweep.GlobalShapeBlending = bool(operation.get("blend", 0))
                sweep.MiterType = 0
                result.extend(sweep.PerformSweepRefit(
                    rail, owned[1] if len(owned) == 2 else owned[1:],
                    operation["refit_tolerance"]) or [])
                return
            result.extend(Rhino.Geometry.Brep.CreateFromSweep(
                rail, owned[1:],
                host["_point"](operation["start_point"]) if "start_point" in operation else Rhino.Geometry.Point3d.Unset,
                host["_point"](operation["end_point"]) if "end_point" in operation else Rhino.Geometry.Point3d.Unset,
                System.Enum.ToObject(Rhino.Geometry.SweepFrame, 1 if operation.get("roadlike", False) or "roadlike_axis" in operation else 0),
                host["_vector"](roadlike_axis) if roadlike_axis is not None else Rhino.Geometry.Vector3d.Unset,
                operation.get("closed", False),
                System.Enum.ToObject(Rhino.Geometry.SweepBlend, operation.get("blend", 0)),
                System.Enum.ToObject(Rhino.Geometry.SweepMiter, 0),
                tolerance["absolute"], System.Enum.ToObject(Rhino.Geometry.SweepRebuild, 2 if operation.get("refit_tolerance") is not None else (1 if operation.get("rebuild_point_count", 0) else 0)),
                operation.get("rebuild_point_count", 0), operation.get("refit_tolerance") or 0.0,
            ) or [])

        _, elapsed = host["_measure"](iterations, build)
        return [face for brep in result for face in host["_sweep1_record"](brep, operation)], elapsed
    finally:
        for geometry in result + owned:
            geometry.Dispose()


def roadlike_axis(operation, rail, tolerance, xyz):
    """Apply documented dialog defaults; SDK Unset has different behavior."""
    axis = operation.get("roadlike_axis")
    if axis is None and operation.get("roadlike", False):
        planar, plane = rail.TryGetPlane(tolerance["absolute"])
        axis = xyz(plane.ZAxis) if planar else [0., 0., 1.]
    return axis


def macro(operation, ids, closed_profiles=False):
    validate(dict(operation, command=True))
    # Rhino's script options differ from the labels in the sweep dialog.
    # Set all supported sticky choices explicitly, independent of prior runs.
    text = "_-Sweep1 '_-SelID %s " % ids[0]
    point_text = lambda value: ",".join(format(float(x), ".17g") for x in value)
    if "start_point" in operation:
        text += " _Point " + point_text(operation["start_point"]) + " "
    text += " ".join("'_-SelID %s" % i for i in ids[1:])
    if "end_point" in operation:
        text += " _Point " + point_text(operation["end_point"])
    text += "" if "end_point" in operation else " _Enter"
    if closed_profiles:
        text += " _Enter"  # Accept the authored closed-profile seams.
    if operation.get("roadlike", False) or "roadlike_axis" in operation:
        text += " _Style=_Roadlike"
        if "roadlike_axis" in operation:
            text += " 0,0,0 " + point_text(operation["roadlike_axis"])
        else:
            text += " _Enter"  # Script getter explicitly accepts CPlane Z.
    else:
        text += " _Style=_Freeform"
    count = operation.get("rebuild_point_count", 0)
    fit = operation.get("refit_tolerance")
    if fit is not None:
        text += " _Simplify=_Refit _RefitTolerance=%.17g" % fit
    else:
        text += " _Simplify=_Rebuild _RebuildCount=%d" % count if count else " _Simplify=_None"
    text += " _Closed=_No _ShapeBlending=_%s _RefitRail=_%s _Enter" % (
        "Global" if operation.get("blend", 0) else "Local",
        "Yes" if operation.get("refit_rail", False) else "No")
    return text


def record(host, brep, operation):
    if not brep.IsValid:
        raise ValueError("Rhino sweep produced an invalid BRep")
    surfaces = []
    for face in brep.Faces:
        samples = []
        if operation.get("queries") is not None:
            for query in operation["queries"]:
                found, u, v = face.ClosestPoint(host["_point"](query))
                if not found:
                    raise ValueError("sweep surface closest point failed")
                samples.append(host["_xyz"](face.PointAt(u, v)))
        else:
            samples = [host["_xyz"](face.PointAt(face.Domain(0).ParameterAt(i/8.0),
                                        face.Domain(1).ParameterAt(j/8.0)))
                       for j in range(9) for i in range(9)]
        record = {"samples": samples}
        if operation.get("inspect_definition", False):
            record["definition"] = host["_nurbs_surface_definition"](face)
        surfaces.append(record)
    return surfaces
