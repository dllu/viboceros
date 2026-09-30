# -*- coding: utf-8 -*-
"""Bounded Zoom fitting captures using owned point objects and public APIs."""
try:
    from .view_camera_probe import _point, _finite, PROJECTIONS
    from .named_view_policy_probe import snapshot
except (ImportError, ValueError):
    from view_camera_probe import _point, _finite, PROJECTIONS
    from named_view_policy_probe import snapshot


METHODS = ("Extents", "Selected", "BoundingBox")
ZOOM_PROJECTIONS = PROJECTIONS + ("Bottom", "Front", "Back", "Right", "Left")


def validate(operation):
    if operation.get("op") != "zoom_extents_probe":
        raise ValueError("unsupported Zoom fitting probe")
    cases = operation.get("cases")
    if not isinstance(cases, list) or not 1 <= len(cases) <= 128:
        raise ValueError("expected 1 to 128 Zoom fitting cases")
    for case in cases:
        if not isinstance(case, dict) or case.get("projection") not in ZOOM_PROJECTIONS:
            raise ValueError("unsupported Zoom fitting projection")
        if case.get("method", "Extents") not in METHODS:
            raise ValueError("unsupported Zoom fitting method")
        minimum = _point(case.get("min"), "min")
        maximum = _point(case.get("max"), "max")
        if any(abs(value) > 1e8 for value in minimum + maximum):
            raise ValueError("Zoom fitting coordinates exceed probe bounds")
        if any(a > b for a, b in zip(minimum, maximum)):
            raise ValueError("reversed Zoom fitting bounds")
        for field, default, low, high in [("border", 1.0, 0.1, 10.0),
                                          ("frustum_scale", 1.0, 0.1, 10.0),
                                          ("vertical_shift", 0.0, -3.0, 3.0)]:
            value = case.get(field, default)
            if (isinstance(value, bool) or not isinstance(value, (int, float))
                    or not _finite(float(value)) or not low <= value <= high):
                raise ValueError("invalid Zoom fitting " + field)
        if "border_command" in case and type(case["border_command"]) is not bool:
            raise ValueError("border_command must be boolean")


def run(operation, viewport, host):
    validate(operation)
    Rhino = host["Rhino"]
    document = Rhino.RhinoDoc.ActiveDoc
    # A live job owns a new document. Refuse to alter or hide foreign geometry.
    if any(True for obj in document.Objects):
        raise ValueError("Zoom fitting probe requires an empty document")
    settings = Rhino.ApplicationSettings.ViewSettings
    fields = ("DefinedViewSetCPlane", "DefinedViewSetProjection",
              "ZoomExtentsParallelViewBorder", "ZoomExtentsPerspectiveViewBorder")
    originals = dict((field, getattr(settings, field)) for field in fields)
    original = Rhino.DocObjects.ViewportInfo(viewport)
    original_target = viewport.CameraTarget
    original_plane = viewport.ConstructionPlane()
    original_name = viewport.Name
    owned = []
    try:
        rows = []
        settings.DefinedViewSetCPlane = True
        settings.DefinedViewSetProjection = True
        for case in operation["cases"]:
            while owned:
                if not document.Objects.Delete(owned[-1], True):
                    raise ValueError("could not delete owned Zoom point")
                owned.pop()
            if not viewport.SetViewProjection(original, False):
                raise ValueError("could not restore Zoom source camera")
            defined = getattr(Rhino.Display.DefinedViewportProjection, case["projection"])
            if not viewport.SetProjection(defined, "Zoom fitting probe", False):
                raise ValueError("could not set Zoom source projection")
            initial_border = 1.3 if case.get("border_command") else case.get("border", 1.0)
            settings.ZoomExtentsParallelViewBorder = initial_border
            settings.ZoomExtentsPerspectiveViewBorder = initial_border
            border_history = None
            if case.get("border_command"):
                history = Rhino.RhinoApp.CommandHistoryWindowText
                border = case.get("border", 1.0)
                if not Rhino.RhinoApp.RunScript(
                        "_SetZoomExtentsBorder _ParallelView=%.17g _PerspectiveView=%.17g _Enter" % (border, border), True):
                    raise ValueError("could not run border settings command")
                updated = Rhino.RhinoApp.CommandHistoryWindowText
                border_history = updated[len(history):][-1500:] if updated.startswith(history) else updated[-1500:]
            effective_border = [float(settings.ZoomExtentsParallelViewBorder),
                                float(settings.ZoomExtentsPerspectiveViewBorder)]
            info = Rhino.DocObjects.ViewportInfo(viewport)
            try:
                scale = case.get("frustum_scale", 1.0)
                shift = case.get("vertical_shift", 0.0)
                half_width = (info.FrustumRight - info.FrustumLeft) * 0.5 * scale
                half_height = (info.FrustumTop - info.FrustumBottom) * 0.5 * scale
                if not info.SetFrustum(-half_width, half_width,
                                       (shift - 1.0) * half_height, (shift + 1.0) * half_height,
                                       info.FrustumNear, info.FrustumFar):
                    raise ValueError("could not set Zoom source frustum")
                if not viewport.SetViewProjection(info, False):
                    raise ValueError("could not apply Zoom source frustum")
            finally:
                info.Dispose()
            minimum, maximum = case["min"], case["max"]
            corners = [Rhino.Geometry.Point3d(*[maximum[axis] if index & (1 << axis)
                                               else minimum[axis] for axis in range(3)])
                       for index in range(8)]
            for point in corners:
                object_id = document.Objects.AddPoint(point)
                if object_id == host["empty_guid"]:
                    raise ValueError("could not add owned Zoom point")
                owned.append(object_id)
            before = snapshot(viewport, Rhino)
            method = case.get("method", "Extents")
            host["progress"]("Zoom %s %s %s" % (method, case["projection"], case.get("id", "")))
            if method == "BoundingBox":
                if viewport.ZoomBoundingBox(Rhino.Geometry.BoundingBox(corners)) is False:
                    raise ValueError("public ZoomBoundingBox failed")
            else:
                if method == "Selected":
                    for object_id in owned:
                        if not document.Objects.Select(object_id):
                            raise ValueError("could not select owned Zoom point")
                if not Rhino.RhinoApp.RunScript("_Zoom _" + method, False):
                    raise ValueError("Zoom command failed")
            after = snapshot(viewport, Rhino)
            after["projected_points"] = []
            for point in corners:
                screen = viewport.WorldToClient(point)
                after["projected_points"].append(dict(point=[float(point.X), float(point.Y), float(point.Z)],
                                                       screen=[float(screen.X), float(screen.Y)]))
            rows.append(dict(case=case, before=before, after=after,
                             effective_border=effective_border, border_history=border_history))
        return rows
    finally:
        cleanup = [("point %s" % object_id, lambda object_id=object_id: document.Objects.Delete(object_id, True))
                   for object_id in owned]
        cleanup.extend([
            ("projection", lambda: viewport.SetViewProjection(original, False)),
            ("target", lambda: viewport.SetCameraTarget(original_target, False)),
            ("construction plane", lambda: viewport.SetConstructionPlane(original_plane)),
            ("name", lambda: setattr(viewport, "Name", original_name)),
        ])
        cleanup.extend([(field, lambda field=field: setattr(settings, field, originals[field]))
                        for field in fields])
        cleanup.append(("viewport info", original.Dispose))
        errors = []
        for label, action in cleanup:
            try:
                if action() is False:
                    raise ValueError("API returned false")
            except Exception as error:
                errors.append("%s: %s" % (label, error))
        if errors:
            raise ValueError("Zoom fitting cleanup failed: " + "; ".join(errors))
