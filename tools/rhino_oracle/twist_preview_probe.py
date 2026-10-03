"""Bounded owned Twist cursor previews with unmodified public geometry and pixels."""

import json
import math
import os
import re


def validate_request(request):
    if (
        not isinstance(request, dict)
        or type(request.get("protocol_version")) is not int
        or request["protocol_version"] != 1
        or type(request.get("iterations", 1)) is not int
        or request.get("iterations", 1) != 1
        or not isinstance(request.get("operations"), list)
        or not 1 <= len(request["operations"]) <= 32
    ):
        raise ValueError("Twist preview requires bounded protocol 1 cases")
    names = set()
    for op in request["operations"]:
        if (
            not isinstance(op, dict)
            or set(op)
            != {
                "op",
                "id",
                "command",
                "copy",
                "shape",
                "rigid",
                "preserve",
                "infinite",
                "phase",
                "view",
                "display_mode",
                "cursor",
                "finish",
            }
            or op["op"] != "twist_preview"
            or op["command"] != "Twist"
            or not isinstance(op["id"], str)
            or re.match(r"^[A-Za-z0-9_-]{1,80}\Z", op["id"]) is None
            or op["id"] in names
            or op["shape"] not in ("Points", "Line", "Curve", "Surface", "Box", "Mesh")
            or any(
                type(op[k]) is not bool
                for k in ("copy", "rigid", "preserve", "infinite")
            )
            or op["phase"] not in ("Target", "Repeat", "Reference", "Wrap")
            or (op["phase"] == "Repeat" and not op["copy"])
            or op["view"] not in ("Top", "Perspective")
            or op["display_mode"] not in ("Wireframe", "Shaded", "Ghosted")
            or op["cursor"] not in ("Valid", "Degenerate")
            or op["finish"] not in ("Click", "Cancel")
            or (op["cursor"] == "Degenerate" and op["finish"] != "Cancel")
            or (op["shape"] == "Box" and op["preserve"])
            or (op["phase"] == "Reference" and op["finish"] != "Cancel")
        ):
            raise ValueError("invalid Twist preview recipe")
        names.add(op["id"])


def request():
    cases = []

    def add(
        shape,
        mode="Wireframe",
        copy=False,
        preserve=False,
        rigid=False,
        infinite=False,
        phase="Target",
        view="Top",
        cursor="Valid",
        finish="Click",
    ):
        cases.append(
            dict(
                op="twist_preview",
                id="Twist_" + str(len(cases)) + "_" + shape + "_" + mode,
                command="Twist",
                shape=shape,
                copy=copy,
                preserve=preserve,
                rigid=rigid,
                infinite=infinite,
                phase=phase,
                view=view,
                display_mode=mode,
                cursor=cursor,
                finish=finish,
            )
        )

    for mode in ("Wireframe", "Shaded", "Ghosted"):
        for shape in ("Line", "Surface", "Box", "Mesh"):
            add(shape, mode=mode)
        for shape in ("Line", "Surface"):
            add(shape, mode=mode, preserve=True)
    add("Points", rigid=True)
    add("Line", copy=True)
    add("Line", copy=True, phase="Repeat", mode="Shaded")
    add("Line", cursor="Degenerate", finish="Cancel")
    add("Line", view="Perspective", mode="Ghosted")
    add("Curve", infinite=True, mode="Shaded")
    add("Line", phase="Reference", finish="Cancel")
    add("Box", phase="Reference", finish="Cancel", mode="Shaded")
    add("Line", rigid=True, mode="Shaded")
    add("Mesh", copy=True, mode="Ghosted")
    add("Box", copy=True)
    add("Surface", cursor="Degenerate", finish="Cancel", mode="Ghosted")
    add("Curve", copy=True, preserve=True)
    add("Line", phase="Wrap")
    return dict(protocol_version=1, iterations=1, operations=cases)


def recipe(op):
    options = " ".join(
        "_" + name + "=_" + ("Yes" if op[key] else "No")
        for name, key in [
            ("Copy", "copy"),
            ("Rigid", "rigid"),
            ("Infinite", "infinite"),
        ]
    )
    if op["shape"] != "Box":
        options += " _PreserveStructure=_" + ("Yes" if op["preserve"] else "No")
    prefix = "_Twist w0,0,0 w0,0,10 " + options + " "
    if op["phase"] == "Repeat":
        prefix += "45 "
    return dict(
        prefix=prefix + ("" if op["phase"] == "Reference" else "w5,0,0 "),
        aim=[0, 5, 0],
        angle=450.0 if op["phase"] == "Wrap" else 90.0,
    )


def run(op, host):
    validate_request(dict(protocol_version=1, operations=[op]))
    Rhino, System = host["Rhino"], host["System"]
    doc = Rhino.RhinoDoc.ActiveDoc
    if list(doc.Objects):
        raise ValueError("twist preview requires an empty owned document")
    import clr

    clr.AddReference("System.Windows.Forms")
    from System.Windows.Forms import Timer
    from shrink_face_input import _input_hooks
    from snap_environment import environment
    from viewport_capture import capture
    from named_view_policy_probe import snapshot as camera_snapshot

    view = doc.Views.ActiveView
    vp = view.ActiveViewport
    original = Rhino.DocObjects.ViewportInfo(vp)
    name, mode, target = vp.Name, vp.DisplayMode, vp.CameraTarget
    root = os.path.dirname(os.path.abspath(host["__file__"]))
    progress = os.path.join(root, "worker-progress.log")
    ready = os.path.join(root, "twist-preview-ready-" + op["id"] + ".json")
    pending, errors, sources, active, witness = [], [], [], [], []
    timer = Timer()
    timer.Interval = 100
    hooks_owned = False

    def snapshot():
        from twist_command_probe import geometry_record

        return [
            dict(
                source=sources.index(obj.Id) if obj.Id in sources else None,
                witness=obj.Id in witness,
                selected=bool(obj.IsSelected(False)),
                geometry=geometry_record(obj.Geometry, host),
            )
            for obj in sorted(
                list(doc.Objects), key=lambda obj: obj.RuntimeSerialNumber
            )
        ]

    def send(line):
        with open(progress, "a") as stream:
            stream.write(line + "\n")
            stream.flush()

    def begun(sender, event):
        if event.CommandEnglishName == op["command"]:
            active.append(True)

    def ended(sender, event):
        if event.CommandEnglishName == op["command"]:
            active[:] = []

    class Mouse(Rhino.UI.MouseCallback):
        def OnEndMouseMove(self, event):
            if (
                not active
                or pending
                or errors
                or not os.path.exists(
                    os.path.join(
                        root, "twist-preview-path-complete-" + op["id"] + ".json"
                    )
                )
                or event.View.ActiveViewport.Id != vp.Id
                or [int(event.ViewportPoint.X), int(event.ViewportPoint.Y)] != [x, y]
                or not Rhino.Input.RhinoGet.InGetPoint(doc)
            ):
                return
            try:
                ok, ray = vp.GetFrustumLine(x, y)
                if not ok:
                    raise ValueError("twist preview ray unavailable")
                frame = capture(vp, host["_point"](aim), [x, y], host)
                frame["ray"] = [host["_xyz"](ray.From), host["_xyz"](ray.To)]
                pending.append(
                    dict(
                        objects=snapshot(),
                        frame=frame,
                        camera=camera_snapshot(vp, Rhino),
                        prompt=Rhino.RhinoApp.CommandPrompt,
                    )
                )
                with open(ready + ".tmp", "w") as stream:
                    json.dump(op["id"], stream)
                os.rename(ready + ".tmp", ready)
            except Exception as error:
                errors.append(str(error))
                send("PICK_ABORT twist-preview-" + op["id"])

    try:
        if not vp.SetProjection(
            getattr(Rhino.Display.DefinedViewportProjection, op["view"]),
            "Owned twist preview",
            False,
        ):
            raise ValueError("twist preview projection failed")
        vp.SetConstructionPlane(Rhino.Geometry.Plane.WorldXY)
        vp.DisplayMode = Rhino.Display.DisplayModeDescription.FindByName(
            op["display_mode"]
        )
        if not vp.ZoomBoundingBox(
            Rhino.Geometry.BoundingBox(
                host["_point"]([-14, -10, -5]), host["_point"]([14, 12, 12])
            )
        ):
            raise ValueError("twist preview zoom failed")
        from twist_command_probe import source_geometry

        owned = source_geometry(op["shape"], host)
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
            raise ValueError("twist preview source insertion failed")
        if op["rigid"] and op["shape"] == "Points":
            doc.Groups.Add("OwnedTwistPreview", sources)
        if op["cursor"] == "Degenerate":
            witness.append(doc.Objects.AddPoint(Rhino.Geometry.Point3d.Origin))
            if witness[0] == System.Guid.Empty:
                raise ValueError("twist preview center witness insertion failed")
        for key in sources:
            doc.Objects.Select(key)
        before = snapshot()
        spec = recipe(op)
        aim = [0, 0, 0] if op["cursor"] == "Degenerate" else spec["aim"]
        pixel = vp.WorldToClient(host["_point"](aim))
        x, y = int(pixel.X), int(pixel.Y)
        if not 1 <= x < vp.Size.Width - 1 or not 1 <= y < vp.Size.Height - 1:
            raise ValueError("twist cursor outside viewport")
        screen = view.ClientToScreen(System.Drawing.Point(x, y))
        corner = view.ClientToScreen(System.Drawing.Point(0, 0))
        valid = vp.WorldToClient(host["_point"](spec["aim"]))
        valid_screen = view.ClientToScreen(
            System.Drawing.Point(int(valid.X), int(valid.Y))
        )
        metadata = dict(
            rect=[
                int(corner.X),
                int(corner.Y),
                int(corner.X + vp.Size.Width),
                int(corner.Y + vp.Size.Height),
            ],
            aim_client=[float(pixel.X), float(pixel.Y)],
            valid_screen=[int(valid_screen.X), int(valid_screen.Y)],
            regions={},
            mouse_path=[],
        )
        for i in range(81 if op["phase"] == "Wrap" else 17):
            angle = math.pi * i / 32.0
            client = vp.WorldToClient(
                host["_point"]([5.0 * math.cos(angle), 5.0 * math.sin(angle), 0.0])
            )
            location = view.ClientToScreen(
                System.Drawing.Point(int(client.X), int(client.Y))
            )
            if (
                not 1 <= client.X < vp.Size.Width - 1
                or not 1 <= client.Y < vp.Size.Height - 1
            ):
                raise ValueError("Twist preview path outside owned viewport")
            metadata["mouse_path"].append([int(location.X), int(location.Y)])
        with open(
            os.path.join(root, "twist-preview-" + op["id"] + ".json"), "w"
        ) as stream:
            json.dump(metadata, stream)
        # Start each owned command with the same remembered scalar; the real
        # mouse path still determines the pending reference angle.
        Rhino.RhinoApp.RunScript(
            "_Twist w0,0,0 w0,0,10 _Copy=_No _Rigid=_No _Infinite=_No 0 _Cancel", False
        )
        history = Rhino.RhinoApp.CommandHistoryWindowText
        sdk_preview = []
        sdk_cubic_preview = []
        if not op["rigid"]:
            from twist_command_probe import geometry_record

            morph = Rhino.Geometry.Morphs.TwistSpaceMorph()
            try:
                morph.TwistAxis = Rhino.Geometry.Line(
                    host["_point"]([0, 0, 0]), host["_point"]([0, 0, 10])
                )
                morph.TwistAngleRadians = math.radians(spec["angle"])
                morph.InfiniteTwist = op["infinite"]
                morph.PreserveStructure = op["preserve"]
                morph.QuickPreview = True
                morph.Tolerance = 1e-5
                for key in sources:
                    source = doc.Objects.FindId(key).Geometry
                    duplicate = (
                        source.ToNurbsCurve()
                        if isinstance(source, Rhino.Geometry.Curve)
                        else source.Duplicate()
                    )
                    try:
                        if not morph.Morph(duplicate):
                            raise ValueError("SDK quick-preview morph failed")
                        record = geometry_record(duplicate, host)
                        if isinstance(duplicate, Rhino.Geometry.Brep):
                            record["edges"] = [
                                geometry_record(edge, host) for edge in duplicate.Edges
                            ]
                        sdk_preview.append(record)
                    finally:
                        duplicate.Dispose()
                    if isinstance(source, Rhino.Geometry.Curve):
                        duplicate = source.ToNurbsCurve()
                        try:
                            if duplicate.Degree < 3 and not duplicate.IncreaseDegree(3):
                                raise ValueError("SDK curve elevation failed")
                            if not morph.Morph(duplicate):
                                raise ValueError("SDK cubic-preview morph failed")
                            sdk_cubic_preview.append(geometry_record(duplicate, host))
                        finally:
                            duplicate.Dispose()
                    elif isinstance(source, Rhino.Geometry.Brep):
                        for face in source.Faces:
                            duplicate = face.ToNurbsSurface()
                            try:
                                if duplicate.Degree(
                                    0
                                ) < 3 and not duplicate.IncreaseDegreeU(3):
                                    raise ValueError("SDK surface U elevation failed")
                                if duplicate.Degree(
                                    1
                                ) < 3 and not duplicate.IncreaseDegreeV(3):
                                    raise ValueError("SDK surface V elevation failed")
                                if not morph.Morph(duplicate):
                                    raise ValueError("SDK cubic surface morph failed")
                                brep = duplicate.ToBrep()
                                try:
                                    sdk_cubic_preview.append(
                                        geometry_record(brep, host)
                                    )
                                finally:
                                    brep.Dispose()
                            finally:
                                duplicate.Dispose()
            finally:
                morph.Dispose()
        with environment(dict(persistent_snaps=["Point"] if witness else []), host):
            hooks_owned = True
            with _input_hooks(
                timer,
                Mouse(),
                [
                    (Rhino.Commands.Command.BeginCommand, begun),
                    (Rhino.Commands.Command.EndCommand, ended),
                ],
            ):
                doc.Views.Redraw()
                send(
                    "PICK @twist-preview:" + op["id"] + " %d %d" % (screen.X, screen.Y)
                )
                success = bool(
                    Rhino.RhinoApp.RunScript(
                        spec["prefix"] + "_Pause" + (" _Enter" if op["copy"] else ""),
                        False,
                    )
                )
        if errors or len(pending) != 1:
            raise ValueError("twist preview incomplete: " + str(errors))
        after = snapshot()
        after_history = Rhino.RhinoApp.CommandHistoryWindowText
        delta = (
            after_history[len(history) :]
            if after_history.startswith(history)
            else after_history[-2000:]
        )
        return (
            dict(
                before=before,
                pending=pending[0],
                after=after,
                success=success,
                history=delta[-2000:],
                calibration=metadata,
                sdk_preview=sdk_preview,
                sdk_cubic_preview=sdk_cubic_preview,
            ),
            0,
        )
    finally:
        if not hooks_owned:
            timer.Dispose()
        for obj in list(doc.Objects):
            doc.Objects.Delete(obj.Id, True)
        vp.SetViewProjection(original, False)
        vp.SetCameraTarget(target, False)
        vp.Name = name
        vp.DisplayMode = mode
        original.Dispose()
        doc.Views.Redraw()
