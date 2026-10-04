"""Bounded owned Maelstrom previews with public quick morphs and private pixels."""
import json
import math
import os
import re


def validate_request(request):
    if (not isinstance(request,dict) or type(request.get("protocol_version")) is not int
        or request["protocol_version"]!=1 or type(request.get("iterations",1)) is not int
        or request.get("iterations",1)!=1 or not isinstance(request.get("operations"),list)
        or not 1<=len(request["operations"])<=32):
        raise ValueError("Maelstrom previews require bounded protocol 1 cases")
    names=set()
    fields={"op","id","shape","copy","rigid","radius0","radius1","plane","phase","view","display_mode","cursor","finish","aim_angle"}
    for op in request["operations"]:
        if (not isinstance(op,dict) or set(op)!=fields or op["op"]!="maelstrom_preview"
            or not isinstance(op["id"],str) or re.match(r"^[A-Za-z0-9_-]{1,80}\Z",op["id"]) is None or op["id"] in names
            or op["shape"] not in ("Points","Line","Curve","Surface","Box","Mesh")
            or any(type(op[k]) is not bool for k in ("copy","rigid"))
            or op["phase"] not in ("First","Second","Angle","Repeat","Wrap")
            or (op["phase"]=="Repeat" and not op["copy"])
            or op["plane"] not in ("XY","Tilt") or op["view"] not in ("Top","Perspective")
            or op["display_mode"] not in ("Wireframe","Shaded","Ghosted")
            or op["cursor"] not in ("Valid","Degenerate") or op["finish"] not in ("Click","Cancel","Coordinate","Number","Axis")
            or (op["phase"] in ("First","Second") and op["finish"]!="Cancel")
            or (op["cursor"]=="Degenerate" and op["finish"] not in ("Click","Cancel"))
            or type(op["aim_angle"]) not in (int,float) or not -90<=op["aim_angle"]<=450
            or (op["phase"]=="Wrap" and op["aim_angle"]!=450)
            or type(op["radius1"]) not in (int,float) or not -10<=op["radius1"]<=10):
            raise ValueError("invalid Maelstrom preview recipe")
        v=op["radius0"]
        if not (type(v) in (int,float) and .01<=v<=10 or isinstance(v,list) and len(v)==3 and all(type(x) in (int,float) and -20<=x<=20 for x in v)):
            raise ValueError("invalid Maelstrom preview first radius")
        if isinstance(v,list):
            center=[1.,2.,3.] if op["plane"]=="Tilt" else [0.,0.,0.]
            if sum((a-b)**2 for a,b in zip(v,center))<.0001:raise ValueError("Maelstrom first point is too close")
        names.add(op["id"])


def request():
    base=dict(op="maelstrom_preview",copy=False,rigid=False,radius0=2.,radius1=5.,plane="XY",phase="Angle",view="Top",display_mode="Wireframe",cursor="Valid",finish="Click",aim_angle=90.)
    changes=[dict(shape=s) for s in ("Points","Line","Curve","Surface","Box","Mesh")]
    changes += [dict(shape=s,display_mode=m) for s in ("Surface","Box","Mesh") for m in ("Shaded","Ghosted")]
    changes += [dict(shape="Line",**c) for c in (dict(finish="Cancel"),dict(copy=True),dict(copy=True,phase="Repeat"),dict(phase="First",finish="Cancel"),dict(phase="Second",finish="Cancel"),dict(cursor="Degenerate",finish="Cancel"))]
    changes += [dict(shape="Points",rigid=True,radius0=1.),dict(shape="Box",rigid=True,radius0=1.),dict(shape="Points",plane="Tilt",view="Perspective"),dict(shape="Line",radius0=[0.,1.,math.sqrt(3.)],view="Perspective"),dict(shape="Line",radius0=[0.,0.,2.],view="Perspective"),dict(shape="Line",aim_angle=135.),dict(shape="Line",phase="Wrap",aim_angle=450.),dict(shape="Line",radius1=0.,finish="Cancel"),dict(shape="Line",radius1=-5.),dict(shape="Line",radius1=2.,aim_angle=60.),dict(shape="Line",radius0=5.,radius1=2.),dict(shape="Mesh",copy=True,display_mode="Ghosted",finish="Cancel"),dict(shape="Line",plane="Tilt",view="Perspective",phase="Second",finish="Cancel"),dict(shape="Box",copy=True,display_mode="Ghosted",finish="Cancel")]
    result=dict(protocol_version=1,iterations=1,operations=[dict(base,id="Maelstrom_preview_"+str(i),**c) for i,c in enumerate(changes)])
    validate_request(result)
    return result


def diagnostic_request():
    source=request()["operations"]
    return dict(protocol_version=1,iterations=1,operations=[source[i] for i in (0,15,16,23,24,25)])


def typed_request():
    source=request()["operations"]
    return dict(protocol_version=1,iterations=1,operations=[
        dict(source[index],id="Maelstrom_typed_hover_"+str(i),shape="Points",finish=finish)
        for i,(index,finish) in enumerate(((24,"Coordinate"),(23,"Coordinate"),(24,"Number")))])


def axis_request():
    source=request()["operations"]
    return dict(protocol_version=1,iterations=1,operations=[
        dict(source[index],id="Maelstrom_axis_hover_"+str(i),shape="Points",finish=finish)
        for i,(index,finish) in enumerate(((24,"Axis"),(0,"Axis"),(17,"Click")))])


def construction_plane(op,host):
    Rhino=host["Rhino"]
    return Rhino.Geometry.Plane(host["_point"]([1.,2.,3.]),Rhino.Geometry.Vector3d(1.,2.,3.)) if op["plane"]=="Tilt" else Rhino.Geometry.Plane.WorldXY


def circle_plane(op,host):
    Rhino=host["Rhino"]
    cplane=construction_plane(op,host)
    if not isinstance(op["radius0"],list):return cplane
    x=host["_point"](op["radius0"])-cplane.Origin
    y=Rhino.Geometry.Vector3d.CrossProduct(cplane.Normal,x)
    if y.Length<=1e-12:y=cplane.YAxis
    return Rhino.Geometry.Plane(cplane.Origin,x,y)


def recipe(op):
    from number_token import number_token
    def token(v):return "w"+",".join(number_token(x) for x in v) if isinstance(v,list) else number_token(v)
    center=[1.,2.,3.] if op["plane"]=="Tilt" else [0.,0.,0.]
    prefix="_Maelstrom "+token(center)+" "
    if op["phase"]!="First":
        prefix+=token(op["radius0"])+" _Copy=_"+("Yes" if op["copy"] else "No")+" _Rigid=_"+("Yes" if op["rigid"] else "No")+" "
    if op["phase"] not in ("First","Second"):prefix+=token(op["radius1"])+" "
    if op["phase"]=="Repeat":prefix+="45 "
    return dict(prefix=prefix)

def run(op, host):
    validate_request(dict(protocol_version=1, operations=[op]))
    Rhino, System = host["Rhino"], host["System"]
    doc = Rhino.RhinoDoc.ActiveDoc
    if list(doc.Objects):
        raise ValueError("Maelstrom preview requires an empty owned document")
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
    captured = os.path.join(root, "maelstrom-preview-captured-" + op["id"] + ".json")
    ready = os.path.join(root, "maelstrom-preview-ready-" + op["id"] + ".json")
    pending, errors, sources, active, witness, arrived, typed_sent = [], [], [], [], [], [], []
    timer = Timer()
    timer.Interval = 100
    hooks_owned = False
    owned_groups = []

    class Mouse(Rhino.UI.MouseCallback):
        def OnEndMouseMove(self, event):
            if (event.View.ActiveViewport.Id == vp.Id
                    and [int(event.ViewportPoint.X), int(event.ViewportPoint.Y)] == [x, y]
                    and os.path.exists(os.path.join(root,"maelstrom-preview-path-complete-"+op["id"]+".json"))):
                arrived[:] = [True]
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
        if event.CommandEnglishName == "Maelstrom":
            active.append(True)
            send("Maelstrom preview " + op["id"] + ": command begun")

    def ended(sender, event):
        if event.CommandEnglishName == "Maelstrom":
            active[:] = []
            send("Maelstrom preview " + op["id"] + ": command ended " + str(event.CommandResult))

    def tick(sender, event):
        try:
            if pending and op["finish"] in ("Coordinate","Number","Axis") and not typed_sent and os.path.exists(captured):
                with open(captured) as stream:
                    if json.load(stream) != op["id"]:
                        raise ValueError("foreign Maelstrom framebuffer receipt")
                from number_token import number_token
                point = metadata["circle"]["origin"] if op["finish"]=="Axis" else metadata["valid_point"]
                if len(point)!=3 or any(not -100<=v<=100 for v in point):
                    raise ValueError("Maelstrom typed point outside bounded calibration")
                token = "-90" if op["finish"]=="Number" else "w"+",".join(number_token(v) for v in point)
                typed_sent.append(True)
                timer.Stop()
                send("Maelstrom preview "+op["id"]+": sending bounded typed input")
                Rhino.RhinoApp.SendKeystrokes(token,True)
            if pending or errors or not active or not arrived or not Rhino.Input.RhinoGet.InGetPoint(doc):
                return
            send("Maelstrom preview " + op["id"] + ": capture pending")
            ok, ray = vp.GetFrustumLine(x, y)
            if not ok:
                raise ValueError("Maelstrom preview ray unavailable")
            frame = capture(vp, host["_point"](aim), [x, y], host)
            frame["ray"] = [host["_xyz"](ray.From), host["_xyz"](ray.To)]
            pending.append(dict(objects=snapshot(), frame=frame,
                                camera=camera_snapshot(vp, Rhino),
                                prompt=Rhino.RhinoApp.CommandPrompt))
            with open(ready + ".tmp", "w") as stream:
                json.dump(op["id"], stream)
            os.rename(ready + ".tmp", ready)
            send("Maelstrom preview " + op["id"] + ": ready")
            if op["finish"] not in ("Coordinate","Number","Axis"):
                timer.Stop()
        except Exception as error:
            errors.append(str(error))
            send("PICK_ABORT maelstrom-preview-" + op["id"])

    try:
        doc.ModelAbsoluteTolerance = 1e-5
        if not vp.SetProjection(getattr(Rhino.Display.DefinedViewportProjection, op["view"]),
                                "Owned Maelstrom preview", False):
            raise ValueError("Maelstrom preview projection failed")
        cplane = construction_plane(op, host)
        vp.SetConstructionPlane(cplane)
        vp.DisplayMode = Rhino.Display.DisplayModeDescription.FindByName(op["display_mode"])
        if not vp.ZoomBoundingBox(Rhino.Geometry.BoundingBox(host["_point"]([-5, -5, -5]),
                                                            host["_point"]([16, 8, 16]))):
            raise ValueError("Maelstrom preview zoom failed")
        center = cplane.Origin
        plane = circle_plane(op, host)
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
            raise ValueError("Maelstrom preview source insertion failed")
        if op["rigid"] and op["shape"] == "Points":
            owned_groups.append(doc.Groups.Add("OwnedMaelstromPreview", sources))
        if op["cursor"] == "Degenerate":
            witness.append(doc.Objects.AddPoint(center))
            if witness[0] == System.Guid.Empty:
                raise ValueError("Maelstrom preview witness insertion failed")
        for key in sources:
            doc.Objects.Select(key)
        before = snapshot()
        spec = recipe(op)
        aim_plane = cplane if op["phase"] == "First" else plane
        angle = math.radians(op["aim_angle"] % 360.)
        spec["aim"] = host["_xyz"](aim_plane.PointAt(5.*math.cos(angle),5.*math.sin(angle)))
        aim = host["_xyz"](center) if witness else spec["aim"]
        pixel = vp.WorldToClient(host["_point"](aim))
        x, y = int(pixel.X), int(pixel.Y)
        if not 1 <= x < vp.Size.Width - 1 or not 1 <= y < vp.Size.Height - 1:
            raise ValueError("Maelstrom cursor outside owned viewport")
        screen = view.ClientToScreen(System.Drawing.Point(x, y))
        corner = view.ClientToScreen(System.Drawing.Point(0, 0))
        valid = vp.WorldToClient(host["_point"](spec["aim"]))
        valid_screen = view.ClientToScreen(System.Drawing.Point(int(valid.X), int(valid.Y)))
        ok, valid_ray = vp.GetFrustumLine(int(valid.X), int(valid.Y))
        if not ok:
            raise ValueError("Maelstrom valid cursor ray unavailable")
        def intersection(origin, normal):
            direction = valid_ray.Direction
            denominator = normal * direction
            if abs(denominator) <= 1e-12:
                return None
            return host["_xyz"](valid_ray.PointAt((normal * (origin - valid_ray.From)) / denominator))
        mouse_plane = cplane if op["phase"] == "First" else plane
        valid_point = intersection(center, mouse_plane.Normal)
        if valid_point is None:
            raise ValueError("Maelstrom calibration plane is edge-on")
        valid_point = host["_point"](valid_point)
        coords = valid_point-center
        principal = math.degrees(math.atan2(coords*plane.YAxis,coords*plane.XAxis))
        degrees = principal + round((op["aim_angle"]-principal)/360.)*360.
        candidates = dict(CPlane=intersection(center,cplane.Normal),Circle=intersection(center,plane.Normal))
        metadata = dict(rect=[int(corner.X), int(corner.Y),
                              int(corner.X + vp.Size.Width), int(corner.Y + vp.Size.Height)],
                        aim_client=[float(pixel.X), float(pixel.Y)],
                        valid_screen=[int(valid_screen.X), int(valid_screen.Y)],
                        valid_point=host["_xyz"](valid_point), candidates=candidates, regions={}, mouse_path=[], degrees=degrees, circle=dict(origin=host["_xyz"](center),x=host["_xyz"](plane.XAxis),y=host["_xyz"](plane.YAxis),normal=host["_xyz"](plane.Normal)))
        for i in range(81 if op["phase"] == "Wrap" else 17):
            angle=math.radians(op["aim_angle"])*i/(80. if op["phase"] == "Wrap" else 16.)
            point=mouse_plane.PointAt(5.*math.cos(angle),5.*math.sin(angle))
            client=vp.WorldToClient(point)
            screen_point=view.ClientToScreen(System.Drawing.Point(int(client.X),int(client.Y)))
            metadata["mouse_path"].append([int(screen_point.X),int(screen_point.Y)])
        with open(os.path.join(root, "maelstrom-preview-" + op["id"] + ".json"), "w") as stream:
            json.dump(metadata, stream)
        history = Rhino.RhinoApp.CommandHistoryWindowText
        sdk_preview, sdk_cubic_preview = [], []
        if op["phase"] in ("Angle","Repeat","Wrap") and not op["rigid"] and abs(op["radius1"]) > 2.**-32:
            radius0 = (host["_point"](op["radius0"])-center).Length if isinstance(op["radius0"],list) else op["radius0"]
            morph = Rhino.Geometry.Morphs.MaelstromSpaceMorph(plane,radius0,abs(op["radius1"]),math.radians(degrees))
            try:
                morph.QuickPreview=True
                morph.PreserveStructure=False
                morph.Tolerance=1e-5
                if not morph.IsValid:raise ValueError("SDK Maelstrom preview definition invalid")
                for key in sources:
                    original_source=doc.Objects.FindId(key).Geometry
                    duplicate=original_source.ToNurbsCurve() if isinstance(original_source,Rhino.Geometry.Curve) else original_source.Duplicate()
                    try:
                        if not morph.Morph(duplicate):raise ValueError("SDK Maelstrom quick morph failed")
                        sdk_preview.append(geometry_record(duplicate,host))
                    finally:duplicate.Dispose()
                    if isinstance(original_source,Rhino.Geometry.Curve):
                        duplicate=original_source.ToNurbsCurve()
                        try:
                            if duplicate.Degree<3 and not duplicate.IncreaseDegree(3):raise ValueError("SDK Maelstrom elevation failed")
                            if not morph.Morph(duplicate):raise ValueError("SDK Maelstrom cubic morph failed")
                            sdk_cubic_preview.append(geometry_record(duplicate,host))
                        finally:duplicate.Dispose()
            finally:morph.Dispose()
        with environment(dict(persistent_snaps=["Point"] if witness else []), host):
            hooks_owned = True
            with _input_hooks(timer, Mouse(), [(timer.Tick, tick), (Rhino.Commands.Command.BeginCommand, begun),
                                           (Rhino.Commands.Command.EndCommand, ended)]):
                doc.Views.Redraw()
                send("PICK @maelstrom-preview:" + op["id"] + " %d %d" % (screen.X, screen.Y))
                success = bool(Rhino.RhinoApp.RunScript(spec["prefix"] + "_Pause"
                                                      + (" _Enter" if op["copy"] else ""), True))
        if errors or len(pending) != 1:
            raise ValueError("Maelstrom preview incomplete: " + str(errors))
        after = snapshot()
        after_history = Rhino.RhinoApp.CommandHistoryWindowText
        delta = after_history[len(history):] if after_history.startswith(history) else after_history[-4000:]
        return dict(before=before, pending=pending[0], after=after, success=success,
                    history=delta[-4000:], calibration=metadata, script_macro=spec["prefix"],
                    sdk_preview=sdk_preview, sdk_cubic_preview=sdk_cubic_preview), 0
    finally:
        if not hooks_owned:
            timer.Dispose()
        for obj in list(doc.Objects):
            doc.Objects.Delete(obj.Id, True)
        for group in owned_groups:
            doc.Groups.Delete(group)
        doc.ModelAbsoluteTolerance = original_tolerance
        vp.SetViewProjection(original, False)
        vp.SetCameraTarget(target, False)
        vp.Name = name
        vp.DisplayMode = mode
        original.Dispose()
        doc.Views.Redraw()
