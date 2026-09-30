# -*- coding: utf-8 -*-
"""Bounded public command-history probe for nested SetView options."""
try:
    from .view_camera_probe import _point, _unit, _xyz
except (ImportError, ValueError):
    from view_camera_probe import _point, _unit, _xyz


TOKENS = ("", "!", "World", "CPlane", "Top", "Bottom", "Front", "Back",
          "Right", "Left", "Perspective", "TwoPointPerspective", "Invalid", "Plan")


def validate(operation):
    if operation.get("op") != "set_view_prompt_probe":
        raise ValueError("unsupported SetView prompt probe")
    for field in ("origin", "x_axis", "y_axis", "camera_target"):
        _point(operation.get(field), field)
    steps = operation.get("steps")
    if (not isinstance(steps, list) or not 1 <= len(steps) <= 64
            or any(not isinstance(step, list) or not 1 <= len(step) <= 8
                   or any(token not in TOKENS for token in step) for step in steps)):
        raise ValueError("SetView probe requires bounded whitelisted token sequences")


def script(tokens):
    if (not isinstance(tokens, list) or not 1 <= len(tokens) <= 8
            or any(token not in TOKENS for token in tokens)):
        raise ValueError("SetView probe requires bounded whitelisted tokens")
    # A final cancel keeps incomplete or rejected input inside the disposable
    # command and prevents the probe from waiting for desktop interaction.
    return "_SetView " + " ".join("_Enter" if token == "" else
                                  "!" if token == "!" else "_" + token
                                  for token in tokens) + " !"


def run(operation, viewport, host):
    validate(operation)
    Rhino = host["Rhino"]
    target = viewport.CameraTarget
    name = viewport.Name
    original_plane = viewport.ConstructionPlane()
    original = Rhino.DocObjects.ViewportInfo(viewport)

    def state():
        plane = viewport.ConstructionPlane()
        return dict(camera_location=_xyz(viewport.CameraLocation),
                    camera_target=_xyz(viewport.CameraTarget),
                    camera_direction=_unit(viewport.CameraDirection),
                    camera_up=_unit(viewport.CameraUp),
                    perspective=bool(viewport.IsPerspectiveProjection),
                    two_point_perspective=bool(viewport.IsTwoPointPerspectiveProjection),
                    cplane_origin=_xyz(plane.Origin), cplane_x=_unit(plane.XAxis),
                    cplane_y=_unit(plane.YAxis))

    try:
        if not viewport.SetProjection(Rhino.Display.DefinedViewportProjection.Perspective,
                                      "SetView prompt probe", False):
            raise ValueError("could not initialize SetView camera")
        if viewport.SetCameraTarget(Rhino.Geometry.Point3d(*operation["camera_target"]), True) is False:
            raise ValueError("could not initialize SetView camera target")
        plane = Rhino.Geometry.Plane(Rhino.Geometry.Point3d(*operation["origin"]),
                                     Rhino.Geometry.Vector3d(*operation["x_axis"]),
                                     Rhino.Geometry.Vector3d(*operation["y_axis"]))
        if not plane.IsValid or viewport.SetConstructionPlane(plane) is False:
            raise ValueError("could not initialize SetView construction plane")
        result = dict(before=state(), steps=[])
        for tokens in operation["steps"]:
            before = Rhino.RhinoApp.CommandHistoryWindowText
            macro = script(tokens)
            host["progress"]("SetView prompt: " + macro)
            succeeded = bool(Rhino.RhinoApp.RunScript(macro, True))
            after = Rhino.RhinoApp.CommandHistoryWindowText
            history = after[len(before):] if after.startswith(before) else after[-3000:]
            result["steps"].append(dict(tokens=tokens, macro=macro, succeeded=succeeded,
                                        history=history, state=state()))
        return result
    finally:
        errors = []
        for label, action in [
            ("projection", lambda: viewport.SetViewProjection(original, False)),
            ("target", lambda: viewport.SetCameraTarget(target, False)),
            ("name", lambda: setattr(viewport, "Name", name)),
            ("construction plane", lambda: viewport.SetConstructionPlane(original_plane)),
            ("viewport info", original.Dispose),
        ]:
            try:
                if action() is False:
                    raise ValueError("API returned false")
            except Exception as error:
                errors.append("%s: %s" % (label, error))
        if errors:
            raise ValueError("SetView probe cleanup failed: " + "; ".join(errors))
