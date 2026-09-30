# -*- coding: utf-8 -*-
"""Bounded Zoom fitting captures using owned point objects and public APIs."""
try:
    from .view_camera_probe import _point, _finite, _xyz, _unit, PROJECTIONS
    from .named_view_policy_probe import snapshot
except (ImportError, ValueError):
    from view_camera_probe import _point, _finite, _xyz, _unit, PROJECTIONS
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
        if "clipping_probe" in case and type(case["clipping_probe"]) is not bool:
            raise ValueError("clipping_probe must be boolean")
        if "projection_probe" in case and type(case["projection_probe"]) is not bool:
            raise ValueError("projection_probe must be boolean")
        if "picking_probe" in case and (type(case["picking_probe"]) is not bool
                                      or not case.get("projection_probe")):
            raise ValueError("picking_probe requires a projection probe and a boolean")
        if "projection_shift" in case:
            shift = case["projection_shift"]
            if (not case.get("projection_probe") or not isinstance(shift, list) or len(shift) != 2
                    or any(isinstance(value, bool) or not isinstance(value, (int, float))
                           or not _finite(float(value)) or abs(value) > 3. for value in shift)):
                raise ValueError("projection_shift requires a projection probe and two bounded numbers")
        if "clip_constraints" in case:
            values = case["clip_constraints"]
            if (not isinstance(values, list) or len(values) != 5
                    or any(isinstance(value, bool) or not isinstance(value, (int, float))
                           or not _finite(float(value)) or abs(value) > 1e12 for value in values)):
                raise ValueError("clip_constraints requires five bounded finite numbers")
        if "context_min" in case or "context_max" in case:
            lo = _point(case.get("context_min"), "context_min")
            hi = _point(case.get("context_max"), "context_max")
            if any(abs(value) > 1e8 for value in lo + hi) or any(a > b for a, b in zip(lo, hi)):
                raise ValueError("invalid context bounds")
        if "context_hidden" in case and type(case["context_hidden"]) is not bool:
            raise ValueError("context_hidden must be boolean")
        if "depth_min" in case or "depth_max" in case:
            lo = _point(case.get("depth_min"), "depth_min")
            hi = _point(case.get("depth_max"), "depth_max")
            if any(abs(value) > 1e8 for value in lo + hi) or any(a > b for a, b in zip(lo, hi)):
                raise ValueError("invalid depth query bounds")
        if "depth_query_before" in case and type(case["depth_query_before"]) is not bool:
            raise ValueError("depth_query_before must be boolean")
        if case.get("depth_query_before") and "depth_min" not in case:
            raise ValueError("depth_query_before requires depth bounds")
        if case.get("display_mode", "Wireframe") not in ("Wireframe", "Shaded", "Ghosted"):
            raise ValueError("unsupported clipping display mode")
        if "redraw_sequence" in case:
            sequence = case["redraw_sequence"]
            if not isinstance(sequence, list) or not 1 <= len(sequence) <= 8:
                raise ValueError("redraw_sequence requires one to eight bounded actions")
            for action in sequence:
                if not isinstance(action, dict) or len(action) != 1:
                    raise ValueError("expected one redraw action")
                field = next(iter(action))
                value = action[field]
                if field in ("zoom_factor", "camera_distance_scale"):
                    if (isinstance(value, bool) or not isinstance(value, (int, float))
                            or not _finite(float(value)) or not 0.1 <= value <= 10.0):
                        raise ValueError("invalid redraw camera scale")
                elif field == "camera_pan":
                    if (not isinstance(value, list) or len(value) != 2
                            or any(isinstance(v, bool) or not isinstance(v, (int, float))
                                   or not _finite(float(v)) or abs(v) > 1e6 for v in value)):
                        raise ValueError("invalid redraw camera pan")
                elif field in ("visible", "delete_geometry", "redraw"):
                    if type(value) is not bool or (field != "visible" and value is not True):
                        raise ValueError("invalid redraw geometry action")
                elif field == "world_view":
                    if value not in ZOOM_PROJECTIONS:
                        raise ValueError("unsupported redraw world view")
                else:
                    raise ValueError("unsupported redraw action")


def info_snapshot(info):
    return dict(camera_location=_xyz(info.CameraLocation),
                camera_target=_xyz(info.TargetPoint),
                camera_direction=_unit(info.CameraDirection), camera_up=_unit(info.CameraUp),
                perspective=bool(info.IsPerspectiveProjection),
                two_point_perspective=bool(info.IsTwoPointPerspectiveProjection),
                frustum=[float(getattr(info, "Frustum" + name))
                         for name in ("Left", "Right", "Bottom", "Top", "Near", "Far")],
                min_near=float(info.PerspectiveMinNearDist),
                min_near_over_far=float(info.PerspectiveMinNearOverFar))


def depth_snapshot(info, case, Rhino):
    box = Rhino.Geometry.BoundingBox(Rhino.Geometry.Point3d(*case["depth_min"]),
                                     Rhino.Geometry.Point3d(*case["depth_max"]))
    intersects, near, far = info.GetBoundingBoxDepth(box)
    return dict(intersects=bool(intersects), near=float(near) if intersects else None,
                far=float(far) if intersects else None)


def projection_snapshot(viewport, corners, Rhino):
    """Read public clip transforms and point visibility without adding geometry."""
    camera = snapshot(viewport, Rhino)
    left, right, bottom, top, near, far = camera["frustum"]
    axes = [_unit(viewport.CameraX), _unit(viewport.CameraY),
            [-v for v in _unit(viewport.CameraZ)]]
    location = camera["camera_location"]
    middle = near ** 0.5 * far ** 0.5
    samples = [("depth-" + str(i), depth, 0., 0.) for i, depth in enumerate(
        [0.5 * near, 0.999 * near, near, 1.001 * near, middle,
         0.999 * far, far, 1.001 * far, 2. * far])]
    for label, x, y in [("left", -1., 0.), ("right", 1., 0.),
                        ("bottom", 0., -1.), ("top", 0., 1.)]:
        for factor in (0.999, 1.001):
            samples.append((label + "-" + str(factor), middle, x * factor, y * factor))
    queries = []
    for label, depth, x, y in samples:
        scale = depth / near if camera["perspective"] else 1.
        x = (0.5 * (left + right) + x * 0.5 * (right - left)) * scale
        y = (0.5 * (bottom + top) + y * 0.5 * (top - bottom)) * scale
        point = [location[i] + axes[0][i] * x + axes[1][i] * y + axes[2][i] * depth
                 for i in range(3)]
        queries.append((label, Rhino.Geometry.Point3d(*point)))
    queries.extend(("corner-" + str(i), point) for i, point in enumerate(corners))
    transform = viewport.GetTransform(Rhino.DocObjects.CoordinateSystem.World,
                                      Rhino.DocObjects.CoordinateSystem.Clip)
    rows = []
    for label, point in queries:
        clip = Rhino.Geometry.Point3d(point)
        clip.Transform(transform)
        screen = viewport.WorldToClient(point)
        rows.append(dict(id=label, point=_xyz(point), clip=_xyz(clip),
                         screen=[float(screen.X), float(screen.Y)],
                         visible=bool(viewport.IsVisible(point))))
    return dict(camera=camera, queries=rows)


def picking_snapshot(viewport, host):
    """Owned line objects and disposable public pick contexts; no redraw."""
    Rhino, System = host["Rhino"], host["System"]
    document = Rhino.RhinoDoc.ActiveDoc
    before = snapshot(viewport, Rhino)
    left, right, bottom, top, near, far = before["frustum"]
    axes = [_unit(viewport.CameraX), _unit(viewport.CameraY),
            [-v for v in _unit(viewport.CameraZ)]]
    middle = near ** 0.5 * far ** 0.5
    def point(depth, x, y=0.):
        scale = depth / near if before["perspective"] else 1.
        x = (0.5 * (left + right) + x * 0.5 * (right - left)) * scale
        y = (0.5 * (bottom + top) + y * 0.5 * (top - bottom)) * scale
        return Rhino.Geometry.Point3d(*[before["camera_location"][i]
            + axes[0][i] * x + axes[1][i] * y + axes[2][i] * depth for i in range(3)])
    sources = [("before-near", 0.5 * near, 0.5 * near, -0.25, 0.25),
               ("inside-near", 1.001 * near, 1.001 * near, -0.25, 0.25),
               ("middle", middle, middle, -0.25, 0.25),
               ("inside-far", 0.999 * far, 0.999 * far, -0.25, 0.25),
               ("after-far", 1.001 * far, 1.001 * far, -0.25, 0.25),
               ("cross-near", 0.5 * near, middle, -0.25, 0.25),
               ("cross-far", middle, 2. * far, -0.25, 0.25),
               ("cross-both", 0.5 * near, 2. * far, -0.25, 0.25),
               ("cross-sides", middle, middle, -2., 2.)]
    owned = []
    records = []
    try:
        for label, a, b, x0, x1 in sources:
            start, end = point(a, x0), point(b, x1)
            object_id = document.Objects.AddLine(start, end)
            if object_id == host["empty_guid"]:
                raise ValueError("could not add owned clipping-pick line")
            owned.append(object_id)
            records.append(dict(id=label, start=_xyz(start), end=_xyz(end)))
        picks = []
        width, height = before["viewport_size"]
        for style in ("PointPick", "WindowPick", "CrossingPick"):
            context = Rhino.Input.Custom.PickContext()
            references = []
            try:
                context.View = document.Views.ActiveView
                context.PickStyle = getattr(Rhino.Input.Custom.PickStyle, style)
                context.PickGroupsEnabled = False
                x, y = width // 2, height // 2
                success, line = viewport.GetFrustumLine(x, y)
                if not success:
                    raise ValueError("could not obtain clipping-pick ray")
                context.PickLine = line
                rect = System.Drawing.Rectangle(x - 8, y - 8, 16, 16) if style == "PointPick" else System.Drawing.Rectangle(1, 1, width - 2, height - 2)
                context.SetPickTransform(viewport.GetPickTransform(rect))
                context.UpdateClippingPlanes()
                primitive = [bool(context.PickFrustumTest(Rhino.Geometry.Line(
                    Rhino.Geometry.Point3d(*record["start"]), Rhino.Geometry.Point3d(*record["end"])))[0])
                    for record in records]
                references = list(document.Objects.PickObjects(context) or [])
                object_ids = set(reference.ObjectId for reference in references)
                picks.append(dict(style=style, rect=[rect.X, rect.Y, rect.Width, rect.Height],
                                  primitive=primitive, objects=[object_id in object_ids for object_id in owned]))
            finally:
                for reference in references:
                    reference.Dispose()
                context.Dispose()
        after = snapshot(viewport, Rhino)
        if before != after:
            raise ValueError("public picking unexpectedly changed its source camera")
        return dict(camera=before, lines=records, picks=picks)
    finally:
        errors = []
        for object_id in owned:
            try:
                if document.Objects.Delete(object_id, True) is False:
                    raise ValueError("could not delete owned clipping-pick line")
            except Exception as error:
                errors.append(str(error))
        if errors:
            raise ValueError("clipping-pick cleanup failed: " + "; ".join(errors))


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
    original_display_mode = viewport.DisplayMode if any("display_mode" in case for case in operation["cases"]) else None
    owned = []
    show_before_delete = any(case.get("context_hidden") or any(
        action.get("visible") is False for action in case.get("redraw_sequence", []))
        for case in operation["cases"])

    def delete_owned(object_id):
        if show_before_delete:
            # Hidden job-owned geometry must be shown before the ordinary
            # Delete overload can remove it. No foreign IDs enter this list.
            document.Objects.Show(object_id, True)
        return document.Objects.Delete(object_id, True)

    try:
        rows = []
        settings.DefinedViewSetCPlane = True
        settings.DefinedViewSetProjection = True
        for case in operation["cases"]:
            while owned:
                if not delete_owned(owned[-1]):
                    raise ValueError("could not delete owned Zoom point")
                owned.pop()
            if not viewport.SetViewProjection(original, False):
                raise ValueError("could not restore Zoom source camera")
            defined = getattr(Rhino.Display.DefinedViewportProjection, case["projection"])
            if not viewport.SetProjection(defined, "Zoom fitting probe", False):
                raise ValueError("could not set Zoom source projection")
            if original_display_mode is not None:
                mode = Rhino.Display.DisplayModeDescription.FindByName(case.get("display_mode", "Wireframe"))
                if mode is None:
                    raise ValueError("missing clipping display mode")
                viewport.DisplayMode = mode
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
            fit_ids = list(owned)
            if "context_min" in case:
                lo, hi = case["context_min"], case["context_max"]
                attributes = Rhino.DocObjects.ObjectAttributes()
                attributes.Visible = not case.get("context_hidden", False)
                for index in range(8):
                    point = Rhino.Geometry.Point3d(*[hi[axis] if index & (1 << axis) else lo[axis] for axis in range(3)])
                    object_id = document.Objects.AddPoint(point, attributes)
                    if object_id == host["empty_guid"]:
                        raise ValueError("could not add owned context point")
                    owned.append(object_id)
            before = snapshot(viewport, Rhino)
            query_before = None
            if case.get("depth_query_before"):
                info = Rhino.DocObjects.ViewportInfo(viewport)
                try:
                    query_before = depth_snapshot(info, case, Rhino)
                finally:
                    info.Dispose()
            method = case.get("method", "Extents")
            host["progress"]("Zoom %s %s %s" % (method, case["projection"], case.get("id", "")))
            if method == "BoundingBox":
                if viewport.ZoomBoundingBox(Rhino.Geometry.BoundingBox(corners)) is False:
                    raise ValueError("public ZoomBoundingBox failed")
            else:
                if method == "Selected":
                    for object_id in fit_ids:
                        if not document.Objects.Select(object_id):
                            raise ValueError("could not select owned Zoom point")
                if not Rhino.RhinoApp.RunScript("_Zoom _" + method, False):
                    raise ValueError("Zoom command failed")
            after = snapshot(viewport, Rhino)
            clipping = None
            if case.get("clipping_probe") or "clip_constraints" in case or "depth_min" in case:
                info = Rhino.DocObjects.ViewportInfo(viewport)
                try:
                    clipping = dict(before=info_snapshot(info))
                    if "depth_min" in case:
                        clipping["depth_query"] = query_before if query_before is not None else depth_snapshot(info, case, Rhino)
                    if "clip_constraints" in case:
                        clipping["succeeded"] = bool(info.SetFrustumNearFar(*case["clip_constraints"]))
                        clipping["after"] = info_snapshot(info)
                finally:
                    info.Dispose()
                if case.get("clipping_probe"):
                    document.Views.Redraw()
                    Rhino.RhinoApp.Wait()
                    clipping["after_redraw"] = snapshot(viewport, Rhino)
            if "redraw_sequence" in case:
                clipping = clipping or {}
                clipping["redraw_steps"] = []
                for action in case["redraw_sequence"]:
                    if "zoom_factor" in action:
                        if not Rhino.RhinoApp.RunScript("_Zoom _Factor %.17g" % action["zoom_factor"], False):
                            raise ValueError("could not run redraw zoom")
                    elif "camera_distance_scale" in action:
                        location, target = _xyz(viewport.CameraLocation), _xyz(viewport.CameraTarget)
                        location = [b + (a - b) * action["camera_distance_scale"]
                                    for a, b in zip(location, target)]
                        viewport.SetCameraLocation(Rhino.Geometry.Point3d(*location), False)
                    elif "camera_pan" in action:
                        right, up = _unit(viewport.CameraX), _unit(viewport.CameraY)
                        x, y = action["camera_pan"]
                        delta = [x * a + y * b for a, b in zip(right, up)]
                        location = [a + b for a, b in zip(_xyz(viewport.CameraLocation), delta)]
                        target = [a + b for a, b in zip(_xyz(viewport.CameraTarget), delta)]
                        viewport.SetCameraLocations(Rhino.Geometry.Point3d(*target),
                                                    Rhino.Geometry.Point3d(*location))
                    elif "visible" in action:
                        method = document.Objects.Show if action["visible"] else document.Objects.Hide
                        for object_id in owned:
                            if method(object_id, True) is False:
                                raise ValueError("could not change owned point visibility")
                    elif "delete_geometry" in action:
                        while owned:
                            if not delete_owned(owned[-1]):
                                raise ValueError("could not delete owned redraw point")
                            owned.pop()
                    elif "world_view" in action:
                        if not Rhino.RhinoApp.RunScript("_SetView _World _" + action["world_view"], False):
                            raise ValueError("could not run redraw world view")
                    before_redraw = snapshot(viewport, Rhino)
                    document.Views.Redraw()
                    Rhino.RhinoApp.Wait()
                    clipping["redraw_steps"].append(dict(action=action, before=before_redraw,
                        after=snapshot(viewport, Rhino)))
            after["projected_points"] = []
            for point in corners:
                screen = viewport.WorldToClient(point)
                after["projected_points"].append(dict(point=[float(point.X), float(point.Y), float(point.Z)],
                                                       screen=[float(screen.X), float(screen.Y)]))
            row = dict(case=case, before=before, after=after,
                       effective_border=effective_border, border_history=border_history,
                       clipping=clipping)
            if case.get("projection_probe"):
                if "projection_shift" in case:
                    info = Rhino.DocObjects.ViewportInfo(viewport)
                    try:
                        x, y = case["projection_shift"]
                        dx = x * (info.FrustumRight - info.FrustumLeft) * 0.5
                        dy = y * (info.FrustumTop - info.FrustumBottom) * 0.5
                        if not info.SetFrustum(info.FrustumLeft + dx, info.FrustumRight + dx,
                                               info.FrustumBottom + dy, info.FrustumTop + dy,
                                               info.FrustumNear, info.FrustumFar):
                            raise ValueError("could not shift projection probe frustum")
                        if not viewport.SetViewProjection(info, False):
                            raise ValueError("could not apply shifted projection probe camera")
                    finally:
                        info.Dispose()
                document.Views.Redraw()
                Rhino.RhinoApp.Wait()
                row["projection"] = projection_snapshot(viewport, corners, Rhino)
                if case.get("picking_probe"):
                    row["picking"] = picking_snapshot(viewport, host)
            rows.append(row)
        return rows
    finally:
        cleanup = [("point %s" % object_id, lambda object_id=object_id: delete_owned(object_id))
                   for object_id in owned]
        cleanup.extend([
            ("projection", lambda: viewport.SetViewProjection(original, False)),
            ("target", lambda: viewport.SetCameraTarget(original_target, False)),
            ("construction plane", lambda: viewport.SetConstructionPlane(original_plane)),
            ("name", lambda: setattr(viewport, "Name", original_name)),
        ])
        cleanup.extend([(field, lambda field=field: setattr(settings, field, originals[field]))
                        for field in fields])
        if original_display_mode is not None:
            cleanup.append(("display mode", lambda: setattr(viewport, "DisplayMode", original_display_mode)))
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
