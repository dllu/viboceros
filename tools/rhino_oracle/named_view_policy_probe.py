# -*- coding: utf-8 -*-
"""Record named-view restoration through the public RhinoCommon API."""
import uuid
try:
    from .view_camera_probe import _point, _unit, _xyz, _finite, PROJECTIONS
except (ImportError, ValueError):
    from view_camera_probe import _point, _unit, _xyz, _finite, PROJECTIONS


def validate(operation):
    if operation.get("op") != "named_view_policy_probe":
        raise ValueError("unsupported named-view policy probe")
    for prefix in ("saved", "current"):
        for field in ("origin", "x_axis", "y_axis", "target"):
            _point(operation.get(prefix + "_" + field), prefix + "_" + field)
    scale = operation.get("saved_frustum_scale", 1.0)
    if (isinstance(scale, bool) or not isinstance(scale, (int, float))
            or not _finite(float(scale)) or not 0.1 <= scale <= 10.0):
        raise ValueError("saved_frustum_scale must be between 0.1 and 10")
    shift = operation.get("saved_frustum_shift", [0.0, 0.0])
    if (not isinstance(shift, list) or len(shift) != 2
            or any(isinstance(value, bool) or not isinstance(value, (int, float))
                   or not _finite(float(value)) or abs(value) > 3.0 for value in shift)):
        raise ValueError("saved_frustum_shift requires two bounded finite numbers")
    policy = operation.get("view_policy")
    if (not isinstance(policy, dict) or set(policy) != set(("set_cplane", "set_projection"))
            or any(type(value) is not bool for value in policy.values())):
        raise ValueError("view_policy requires boolean set_cplane and set_projection")
    for field in ("saved_projections", "current_projections"):
        values = operation.get(field)
        if (not isinstance(values, list) or not 1 <= len(values) <= len(PROJECTIONS)
                or any(value not in PROJECTIONS for value in values)
                or len(set(values)) != len(values)):
            raise ValueError("named-view policy projections must be distinct supported names")


def snapshot(viewport, Rhino):
    plane = viewport.ConstructionPlane()
    info = Rhino.DocObjects.ViewportInfo(viewport)
    try:
        points = []
        target = _xyz(viewport.CameraTarget)
        for offset in ([0, 0, 0], [3, -2, 4], [-5, 7, -3]):
            point = [a + b for a, b in zip(target, offset)]
            screen = viewport.WorldToClient(Rhino.Geometry.Point3d(*point))
            points.append(dict(point=point, screen=[float(screen.X), float(screen.Y)]))
        return dict(camera_location=_xyz(viewport.CameraLocation),
                    camera_target=_xyz(viewport.CameraTarget),
                    camera_direction=_unit(viewport.CameraDirection),
                    camera_up=_unit(viewport.CameraUp),
                    cplane_origin=_xyz(plane.Origin), cplane_x=_unit(plane.XAxis),
                    cplane_y=_unit(plane.YAxis),
                    perspective=bool(viewport.IsPerspectiveProjection),
                    two_point_perspective=bool(viewport.IsTwoPointPerspectiveProjection),
                    frustum=[float(getattr(info, "Frustum" + name))
                             for name in ("Left", "Right", "Bottom", "Top", "Near", "Far")],
                    viewport_size=[int(viewport.Size.Width), int(viewport.Size.Height)],
                    projected_points=points)
    finally:
        info.Dispose()


def run(operation, viewport, host):
    validate(operation)
    Rhino = host["Rhino"]
    table = Rhino.RhinoDoc.ActiveDoc.NamedViews
    settings = Rhino.ApplicationSettings.ViewSettings
    policy = dict(set_cplane=bool(settings.DefinedViewSetCPlane),
                  set_projection=bool(settings.DefinedViewSetProjection))
    original_target = viewport.CameraTarget
    original_name = viewport.Name
    original_plane = viewport.ConstructionPlane()
    original = Rhino.DocObjects.ViewportInfo(viewport)
    owned = []

    def initialize(prefix, projection):
        settings.DefinedViewSetProjection = True
        settings.DefinedViewSetCPlane = True
        defined = getattr(Rhino.Display.DefinedViewportProjection, projection)
        if not viewport.SetProjection(defined, "Named view policy probe", False):
            raise ValueError("could not initialize named-view probe projection")
        if viewport.SetCameraTarget(Rhino.Geometry.Point3d(*operation[prefix + "_target"]), True) is False:
            raise ValueError("could not initialize named-view probe target")
        plane = Rhino.Geometry.Plane(Rhino.Geometry.Point3d(*operation[prefix + "_origin"]),
                                     Rhino.Geometry.Vector3d(*operation[prefix + "_x_axis"]),
                                     Rhino.Geometry.Vector3d(*operation[prefix + "_y_axis"]))
        if not plane.IsValid or viewport.SetConstructionPlane(plane) is False:
            raise ValueError("could not initialize named-view probe plane")

    try:
        rows = []
        for saved_projection in operation["saved_projections"]:
            initialize("saved", saved_projection)
            if "saved_frustum_scale" in operation or "saved_frustum_shift" in operation:
                info = Rhino.DocObjects.ViewportInfo(viewport)
                try:
                    scale = operation.get("saved_frustum_scale", 1.0)
                    half_width = (info.FrustumRight - info.FrustumLeft) * 0.5 * scale
                    half_height = (info.FrustumTop - info.FrustumBottom) * 0.5 * scale
                    x, y = operation.get("saved_frustum_shift", [0.0, 0.0])
                    if not info.SetFrustum((x - 1.0) * half_width, (x + 1.0) * half_width,
                                           (y - 1.0) * half_height, (y + 1.0) * half_height,
                                           info.FrustumNear, info.FrustumFar):
                        raise ValueError("could not set saved probe frustum")
                    if not viewport.SetViewProjection(info, False):
                        raise ValueError("could not apply saved probe frustum")
                finally:
                    info.Dispose()
            saved = snapshot(viewport, Rhino)
            index = table.Add("VibocerosOracle_" + uuid.uuid4().hex, viewport.Id)
            if index < 0:
                raise ValueError("could not add disposable named view")
            owned.append(index)
            for current_projection in operation["current_projections"]:
                initialize("current", current_projection)
                before = snapshot(viewport, Rhino)
                settings.DefinedViewSetCPlane = operation["view_policy"]["set_cplane"]
                settings.DefinedViewSetProjection = operation["view_policy"]["set_projection"]
                host["progress"]("named view %s -> %s" % (saved_projection, current_projection))
                if not table.Restore(index, viewport):
                    raise ValueError("could not restore disposable named view")
                rows.append(dict(saved_projection=saved_projection,
                                 current_projection=current_projection,
                                 view_policy=operation["view_policy"],
                                 saved=saved, before=before, after=snapshot(viewport, Rhino)))
        return rows
    finally:
        cleanup = [("named view %s" % index, lambda index=index: table.Delete(index))
                   for index in reversed(owned)]
        cleanup.extend([
            ("projection", lambda: viewport.SetViewProjection(original, False)),
            ("target", lambda: viewport.SetCameraTarget(original_target, False)),
            ("construction plane", lambda: viewport.SetConstructionPlane(original_plane)),
            ("name", lambda: setattr(viewport, "Name", original_name)),
            ("CPlane policy", lambda: setattr(settings, "DefinedViewSetCPlane", policy["set_cplane"])),
            ("projection policy", lambda: setattr(settings, "DefinedViewSetProjection", policy["set_projection"])),
            ("viewport info", original.Dispose),
        ])
        errors = []
        for label, action in cleanup:
            try:
                if action() is False:
                    raise ValueError("API returned false")
            except Exception as error:
                errors.append("%s: %s" % (label, error))
        if errors:
            raise ValueError("named view probe cleanup failed: " + "; ".join(errors))
