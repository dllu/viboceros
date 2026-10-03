"""Bounded owned Bend cursor previews, public quick morphs and raw pixels."""

import json
import math
import os
import re


FLAGS = (("Copy", "copy"), ("Rigid", "rigid"), ("LimitToSpine", "limited"),
         ("Symmetric", "symmetric"), ("PreserveStructure", "preserve"),
         ("NonAttenuated", "uniform"))


def validate_request(request):
    if (not isinstance(request, dict)
            or type(request.get("protocol_version")) is not int
            or request["protocol_version"] != 1
            or type(request.get("iterations", 1)) is not int
            or request.get("iterations", 1) != 1
            or not isinstance(request.get("operations"), list)
            or not 1 <= len(request["operations"]) <= 32):
        raise ValueError("Bend preview requires bounded protocol 1 cases")
    names = set()
    for op in request["operations"]:
        if (not isinstance(op, dict)
                or set(op) - {"spine_z"} != {"op", "id", "shape", "angle", "phase", "view",
                               "display_mode", "cursor", "finish"} | {k for _, k in FLAGS}
                or op["op"] != "bend_preview"
                or not isinstance(op["id"], str)
                or re.match(r"^[A-Za-z0-9_-]{1,80}\Z", op["id"]) is None
                or op["id"] in names
                or op["shape"] not in ("Points", "Line", "Curve", "Surface", "Box", "Mesh")
                or any(type(op[k]) is not bool for _, k in FLAGS)
                or type(op["angle"]) not in (int, float)
                or not 0 <= op["angle"] <= 360
                or type(op.get("spine_z", 0.0)) not in (int, float)
                or not -10 <= op.get("spine_z", 0.0) <= 10
                or op["phase"] not in ("Through", "Repeat")
                or (op["phase"] == "Repeat" and not op["copy"])
                or op["view"] not in ("Front", "Perspective", "Top")
                or op["display_mode"] not in ("Wireframe", "Shaded", "Ghosted")
                or op["cursor"] not in ("Valid", "Degenerate", "Short")
                or op["finish"] not in ("Click", "Cancel")
                or (op["cursor"] == "Degenerate" and op["finish"] != "Cancel")
                or (op["shape"] == "Box" and op["preserve"])):
            raise ValueError("invalid Bend preview recipe")
        names.add(op["id"])


def request():
    cases = []

    def add(shape, **options):
        op = dict(op="bend_preview", id="Bend_" + str(len(cases)) + "_" + shape,
                  shape=shape, copy=False, rigid=False, limited=False,
                  symmetric=False, preserve=False, uniform=False, angle=0.0,
                  phase="Through", view="Front", display_mode="Wireframe",
                  cursor="Valid", finish="Click")
        op.update(options)
        cases.append(op)

    for mode in ("Wireframe", "Shaded", "Ghosted"):
        for shape in ("Line", "Surface", "Box", "Mesh"):
            add(shape, display_mode=mode)
        add("Surface", display_mode=mode, preserve=True)
    add("Points", rigid=True)
    add("Curve", preserve=True)
    add("Line", copy=True)
    add("Line", copy=True, phase="Repeat", display_mode="Shaded")
    add("Line", cursor="Degenerate", finish="Cancel")
    add("Surface", cursor="Degenerate", finish="Cancel", display_mode="Ghosted")
    add("Line", limited=True)
    add("Points", symmetric=True, uniform=True)
    add("Line", angle=90, limited=True)
    add("Line", uniform=True)
    add("Box", rigid=True, display_mode="Shaded")
    add("Line", view="Perspective", display_mode="Ghosted")
    return dict(protocol_version=1, iterations=1, operations=cases)


def recipe(op):
    z = op.get("spine_z", 0.0)
    options = " ".join("_" + name + "=_" + ("Yes" if op[key] else "No")
                       for name, key in FLAGS if name != "PreserveStructure" or op["shape"] != "Box")
    prefix = "_Bend w0,0," + str(z) + " w0,0," + str(z + 10) + " " + options + " _Angle " + str(float(op["angle"])) + " "
    if op["phase"] == "Repeat":
        prefix += "w5,0," + str(z + 10) + " "
    return dict(prefix=prefix, aim=[5.0, 0.0, z + 5.0] if op["cursor"] == "Short" else [10.0, 0.0, z + 10.0])


def followup_request():
    base = request()
    changes = [dict(preserve=True), dict(copy=True, finish="Cancel"),
               dict(limited=True, cursor="Short"), dict(symmetric=True, cursor="Short"),
               dict(view="Top", shape="Points"),
               dict(view="Top", shape="Box", display_mode="Shaded"),
               dict(view="Top", angle=90, limited=True),
               dict(view="Top", shape="Points", spine_z=3.0)]
    return dict(base, operations=[dict(base["operations"][0], id="Bend_followup_" + str(i), **change)
                                 for i, change in enumerate(changes)])


def run(op, host):
    validate_request(dict(protocol_version=1, operations=[op]))
    Rhino, System = host["Rhino"], host["System"]
    doc = Rhino.RhinoDoc.ActiveDoc
    if list(doc.Objects):
        raise ValueError("Bend preview requires an empty owned document")
    import clr
    clr.AddReference("System.Windows.Forms")
    from System.Windows.Forms import Timer
    from shrink_face_input import _input_hooks
    from snap_environment import environment
    from viewport_capture import capture
    from named_view_policy_probe import snapshot as camera_snapshot
    from twist_command_probe import geometry_record, source_geometry

    view = doc.Views.ActiveView
    vp = view.ActiveViewport
    original = Rhino.DocObjects.ViewportInfo(vp)
    name, mode, target = vp.Name, vp.DisplayMode, vp.CameraTarget
    original_tolerance = doc.ModelAbsoluteTolerance
    root = os.path.dirname(os.path.abspath(host["__file__"]))
    progress = os.path.join(root, "worker-progress.log")
    captured = os.path.join(root, "bend-preview-captured-" + op["id"] + ".json")
    ready = os.path.join(root, "bend-preview-ready-" + op["id"] + ".json")
    pending, errors, sources, active, witness = [], [], [], [], []
    timer = Timer()
    timer.Interval = 100
    hooks_owned = False

    class Mouse(Rhino.UI.MouseCallback):
        def OnEndMouseMove(self, event):
            if (event.View.ActiveViewport.Id == vp.Id
                    and [int(event.ViewportPoint.X), int(event.ViewportPoint.Y)] == [x, y]):
                tick(None, None)

    def snapshot():
        return [dict(source=sources.index(obj.Id) if obj.Id in sources else None,
                     witness=obj.Id in witness, selected=bool(obj.IsSelected(False)),
                     geometry=geometry_record(obj.Geometry, host))
                for obj in sorted(list(doc.Objects), key=lambda obj: obj.RuntimeSerialNumber)]

    def send(line):
        with open(progress, "a") as stream:
            stream.write(line + "\n")
            stream.flush()

    def begun(sender, event):
        if event.CommandEnglishName == "Bend":
            active.append(True)
            send("Bend preview " + op["id"] + ": command begun")

    def ended(sender, event):
        if event.CommandEnglishName == "Bend":
            active[:] = []
            send("Bend preview " + op["id"] + ": command ended " + str(event.CommandResult))

    def tick(sender, event):
        if pending or errors or not active or not Rhino.Input.RhinoGet.InGetPoint(doc):
            return
        try:
            send("Bend preview " + op["id"] + ": capture pending")
            ok, ray = vp.GetFrustumLine(x, y)
            if not ok:
                raise ValueError("Bend preview ray unavailable")
            frame = capture(vp, host["_point"](aim), [x, y], host)
            frame["ray"] = [host["_xyz"](ray.From), host["_xyz"](ray.To)]
            pending.append(dict(objects=snapshot(), frame=frame,
                                camera=camera_snapshot(vp, Rhino),
                                prompt=Rhino.RhinoApp.CommandPrompt))
            with open(ready + ".tmp", "w") as stream:
                json.dump(op["id"], stream)
            os.rename(ready + ".tmp", ready)
            send("Bend preview " + op["id"] + ": ready")
            timer.Stop()
        except Exception as error:
            errors.append(str(error))
            send("PICK_ABORT bend-preview-" + op["id"])

    try:
        doc.ModelAbsoluteTolerance = 1e-5
        if not vp.SetProjection(getattr(Rhino.Display.DefinedViewportProjection, op["view"]),
                                "Owned Bend preview", False):
            raise ValueError("Bend preview projection failed")
        vp.SetConstructionPlane(Rhino.Geometry.Plane.WorldXY if op["view"] == "Top" else Rhino.Geometry.Plane.WorldZX)
        vp.DisplayMode = Rhino.Display.DisplayModeDescription.FindByName(op["display_mode"])
        if not vp.ZoomBoundingBox(Rhino.Geometry.BoundingBox(host["_point"]([-5, -5, -5]),
                                                            host["_point"]([16, 8, 16]))):
            raise ValueError("Bend preview zoom failed")
        owned = source_geometry(op["shape"], host)
        for geom in owned:
            if not geom.Transform(Rhino.Geometry.Transform.Translation(0, 0, op.get("spine_z", 0.0))):
                raise ValueError("Bend preview source translation failed")
        attrs = Rhino.DocObjects.ObjectAttributes()
        attrs.ColorSource = Rhino.DocObjects.ObjectColorSource.ColorFromObject
        attrs.ObjectColor = System.Drawing.Color.FromArgb(200, 80, 60)
        try:
            for geom in owned:
                sources.append(doc.Objects.Add(geom, attrs))
        finally:
            attrs.Dispose()
            for geom in owned:
                geom.Dispose()
        if any(key == System.Guid.Empty for key in sources):
            raise ValueError("Bend preview source insertion failed")
        if op["rigid"] and op["shape"] == "Points":
            doc.Groups.Add("OwnedBendPreview", sources)
        if op["cursor"] == "Degenerate":
            witness.append(doc.Objects.AddPoint(host["_point"]([0, 0, 10])))
            if witness[0] == System.Guid.Empty:
                raise ValueError("Bend preview witness insertion failed")
        for key in sources:
            doc.Objects.Select(key)
        before = snapshot()
        spec = recipe(op)
        aim = [0, 0, 10] if witness else spec["aim"]
        pixel = vp.WorldToClient(host["_point"](aim))
        x, y = int(pixel.X), int(pixel.Y)
        if not 1 <= x < vp.Size.Width - 1 or not 1 <= y < vp.Size.Height - 1:
            raise ValueError("Bend cursor outside owned viewport")
        screen = view.ClientToScreen(System.Drawing.Point(x, y))
        corner = view.ClientToScreen(System.Drawing.Point(0, 0))
        valid = vp.WorldToClient(host["_point"](spec["aim"]))
        valid_screen = view.ClientToScreen(System.Drawing.Point(int(valid.X), int(valid.Y)))
        ok, valid_ray = vp.GetFrustumLine(int(valid.X), int(valid.Y))
        normal_axis = 2 if op["view"] == "Top" else 1
        plane_coordinate = op.get("spine_z", 0.0) if op["view"] == "Top" else 0.0
        if not ok or valid_ray.From[normal_axis] == valid_ray.To[normal_axis]:
            raise ValueError("Bend valid cursor has no construction-plane intersection")
        t = (plane_coordinate - valid_ray.From[normal_axis]) / (valid_ray.To[normal_axis] - valid_ray.From[normal_axis])
        valid_point = valid_ray.PointAt(t)
        metadata = dict(rect=[int(corner.X), int(corner.Y),
                              int(corner.X + vp.Size.Width), int(corner.Y + vp.Size.Height)],
                        aim_client=[float(pixel.X), float(pixel.Y)],
                        valid_screen=[int(valid_screen.X), int(valid_screen.Y)],
                        valid_point=host["_xyz"](valid_point), regions={})
        with open(os.path.join(root, "bend-preview-" + op["id"] + ".json"), "w") as stream:
            json.dump(metadata, stream)
        history = Rhino.RhinoApp.CommandHistoryWindowText
        sdk_preview, sdk_cubic_preview = [], []
        # These unlimited, attenuated command recipes have the same public SDK
        # construction. Limited/uniform commands retain their actual pixels and
        # terminal geometry without claiming an equivalent SDK configuration.
        if not op["rigid"] and not op["limited"] and not op["uniform"]:
            z = op.get("spine_z", 0.0)
            args = [host["_point"]([0, 0, z]), host["_point"]([0, 0, z + 10]), valid_point]
            if op["angle"] != 0:
                args.append(math.radians(op["angle"]))
            args.extend([False, op["symmetric"]])
            morph = Rhino.Geometry.Morphs.BendSpaceMorph(*args)
            try:
                morph.QuickPreview = True
                morph.PreserveStructure = op["preserve"]
                morph.Tolerance = 1e-5
                if not morph.IsValid:
                    raise ValueError("SDK Bend preview definition is invalid")
                for key in sources:
                    source = doc.Objects.FindId(key).Geometry
                    duplicate = source.ToNurbsCurve() if isinstance(source, Rhino.Geometry.Curve) else source.Duplicate()
                    try:
                        if not morph.Morph(duplicate):
                            raise ValueError("SDK Bend quick morph failed")
                        sdk_preview.append(geometry_record(duplicate, host))
                    finally:
                        duplicate.Dispose()
                    if isinstance(source, Rhino.Geometry.Curve):
                        duplicate = source.ToNurbsCurve()
                        try:
                            if duplicate.Degree < 3 and not duplicate.IncreaseDegree(3):
                                raise ValueError("SDK Bend cubic elevation failed")
                            if not morph.Morph(duplicate):
                                raise ValueError("SDK Bend cubic morph failed")
                            sdk_cubic_preview.append(geometry_record(duplicate, host))
                        finally:
                            duplicate.Dispose()
            finally:
                morph.Dispose()
        with environment(dict(persistent_snaps=["Point"] if witness else []), host):
            hooks_owned = True
            with _input_hooks(timer, Mouse(), [(Rhino.Commands.Command.BeginCommand, begun),
                                           (Rhino.Commands.Command.EndCommand, ended)]):
                doc.Views.Redraw()
                send("PICK @bend-preview:" + op["id"] + " %d %d" % (screen.X, screen.Y))
                success = bool(Rhino.RhinoApp.RunScript(spec["prefix"] + "_Pause"
                                                      + (" _Enter" if op["copy"] else ""), True))
        if errors or len(pending) != 1:
            raise ValueError("Bend preview incomplete: " + str(errors))
        after = snapshot()
        after_history = Rhino.RhinoApp.CommandHistoryWindowText
        delta = after_history[len(history):] if after_history.startswith(history) else after_history[-4000:]
        return dict(before=before, pending=pending[0], after=after, success=success,
                    history=delta[-4000:], calibration=metadata,
                    sdk_preview=sdk_preview, sdk_cubic_preview=sdk_cubic_preview), 0
    finally:
        if not hooks_owned:
            timer.Dispose()
        for obj in list(doc.Objects):
            doc.Objects.Delete(obj.Id, True)
        doc.ModelAbsoluteTolerance = original_tolerance
        vp.SetViewProjection(original, False)
        vp.SetCameraTarget(target, False)
        vp.Name = name
        vp.DisplayMode = mode
        original.Dispose()
        doc.Views.Redraw()
