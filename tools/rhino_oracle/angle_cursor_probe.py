"""Owned Rhino Line prompt with a typed angle lock and a real mouse pick."""

import math
import os
import re

def finite(value):
    return type(value) in (int, float) and not math.isnan(value) and not math.isinf(value)


def validate_request(request):
    operations = request.get("operations")
    if (request.get("protocol_version") != 1 or request.get("iterations", 1) != 1
            or not isinstance(operations, list) or not 1 <= len(operations) <= 32):
        raise ValueError("angle cursor probe needs protocol 1 and one iteration")
    seen = set()
    for operation in operations:
        if not isinstance(operation, dict) or set(operation) != {"op", "id", "angle", "aim"}:
            raise ValueError("invalid angle cursor probe fields")
        name = operation["id"]
        if operation["op"] != "angle_cursor_diagnostic" or not isinstance(name, (str, type(u""))) or re.match(r"^[A-Za-z0-9_-]{1,80}\Z", name) is None or name in seen:
            raise ValueError("invalid angle cursor probe id")
        seen.add(name)
        angle = operation["angle"]
        aim = operation["aim"]
        if ((angle is not None and (not finite(angle) or abs(angle) > 10000))
                or not isinstance(aim, list) or len(aim) != 3
                or any(not finite(value) or abs(value) > 100 for value in aim)):
            raise ValueError("invalid angle cursor probe angle or aim")


def run(operation, host):
    validate_request({"protocol_version": 1, "operations": [operation]})
    Rhino, System = host["Rhino"], host["System"]
    document = Rhino.RhinoDoc.ActiveDoc
    view = document.Views.ActiveView
    viewport = view.ActiveViewport
    original_projection = Rhino.DocObjects.ViewportInfo(viewport)
    original_name, original_target = viewport.Name, viewport.CameraTarget
    aid = Rhino.ApplicationSettings.ModelAidSettings
    track = Rhino.ApplicationSettings.SmartTrackSettings
    original_aid, original_track = aid.GetCurrentState(), track.GetCurrentState()
    before = {obj.Id for obj in document.Objects}
    selected = [obj.Id for obj in document.Objects.GetSelectedObjects(False, False)]
    sent = []
    progress = os.path.join(os.path.dirname(os.path.abspath(host["__file__"])), "worker-progress.log")
    try:
        if before:
            raise ValueError("angle cursor probe needs an empty document")
        aid.Ortho = aid.GridSnap = aid.Osnap = False
        track.UseSmartTrack = False
        if not viewport.SetProjection(Rhino.Display.DefinedViewportProjection.Top, "Angle probe", False):
            raise ValueError("could not set Top projection")
        bounds = Rhino.Geometry.BoundingBox(host["_point"]([-12, -12, -1]), host["_point"]([12, 12, 1]))
        if not viewport.ZoomBoundingBox(bounds):
            raise ValueError("could not fit angle cursor probe view")
        document.Views.Redraw()
        pixel = viewport.WorldToClient(host["_point"](operation["aim"]))
        x, y = int(pixel.X), int(pixel.Y)
        if not 1 <= x < viewport.Size.Width - 1 or not 1 <= y < viewport.Size.Height - 1:
            raise ValueError("angle cursor target outside owned viewport")
        screen = view.ClientToScreen(System.Drawing.Point(x, y))
        with open(progress, "a") as stream:
            stream.write("PICK @angle:%s %d %d\n" % (operation["id"], screen.X, screen.Y))
            stream.flush()
        sent.append([x, y])
        script = ("_Line w0,0,0 <%.17g _Pause" % operation["angle"]
                  if operation["angle"] is not None else "_Line w0,0,0 _Pause")
        before_history = Rhino.RhinoApp.CommandHistoryWindowText
        success = Rhino.RhinoApp.RunScript(script, False)
        if not success:
            raise ValueError("angle cursor Line prompt failed")
        created = [obj for obj in document.Objects if obj.Id not in before]
        if len(created) != 1 or not isinstance(created[0].Geometry, Rhino.Geometry.Curve):
            raise ValueError("angle cursor prompt did not create one curve")
        curve = created[0].Geometry
        after_history = Rhino.RhinoApp.CommandHistoryWindowText
        history = after_history[len(before_history):] if after_history.startswith(before_history) else after_history[-2000:]
        return {"start": host["_xyz"](curve.PointAtStart),
                "end": host["_xyz"](curve.PointAtEnd),
                "click_client": sent[0], "history": history[-2000:]}, 0
    finally:
        for obj in list(document.Objects):
            if obj.Id not in before:
                document.Objects.Delete(obj.Id, True)
        viewport.SetViewProjection(original_projection, False)
        viewport.SetCameraTarget(original_target, False)
        viewport.Name = original_name
        aid.UpdateFromState(original_aid)
        track.UpdateFromState(original_track)
        document.Objects.UnselectAll()
        for key in selected:
            document.Objects.Select(key)
