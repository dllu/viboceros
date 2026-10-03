"""Owned Taper cursor prompts, independent quick morphs and private pixels."""
import json
import math
import os
import re

FLAGS=(("Copy","copy"),("Rigid","rigid"),("Flat","flat"),("Infinite","infinite"),("PreserveStructure","preserve"))
AXES={"Z":([0.,0.,0.],[0.,0.,10.]),"Tilt":([0.,0.,0.],[5.,0.,10.]),"Spatial":([1.,2.,3.],[5.,6.,11.])}


def axis(op):
    a,b=AXES[op.get("axis","Z")]
    z=op.get("spine_z",0.)
    return [a[0],a[1],a[2]+z],[b[0],b[1],b[2]+z]


def validate_request(request):
    if (not isinstance(request,dict) or type(request.get("protocol_version")) is not int
        or request["protocol_version"]!=1 or type(request.get("iterations",1)) is not int
        or request.get("iterations",1)!=1 or not isinstance(request.get("operations"),list)
        or not 1<=len(request["operations"])<=32):
        raise ValueError("Taper preview requires bounded protocol 1 cases")
    names=set()
    for op in request["operations"]:
        if (not isinstance(op,dict) or set(op)-{"spine_z","axis"}!={"op","id","shape","initial","phase","view","display_mode","cursor","finish"}|{k for _,k in FLAGS}
            or op["op"]!="taper_preview" or not isinstance(op["id"],str)
            or re.match(r"^[A-Za-z0-9_-]{1,80}\Z",op["id"]) is None or op["id"] in names
            or op["shape"] not in ("Points","Line","Curve","Surface","Box","Mesh")
            or any(type(op[k]) is not bool for _,k in FLAGS)
            or not isinstance(op.get("axis","Z"),str) or op.get("axis","Z") not in AXES
            or type(op.get("spine_z",0.)) not in (int,float) or not -10<=op.get("spine_z",0.)<=10
            or op["phase"] not in ("Start","End","Repeat") or (op["phase"]=="Repeat" and not op["copy"])
            or op["view"] not in ("Front","Perspective","Top")
            or op["display_mode"] not in ("Wireframe","Shaded","Ghosted")
            or op["cursor"] not in ("Valid","Degenerate") or op["finish"] not in ("Click","Cancel")
            or ((op["cursor"]=="Degenerate" or op["phase"]=="Start") and op["finish"]!="Cancel")
            or (op["shape"]=="Box" and op["preserve"])):
            raise ValueError("invalid Taper preview recipe")
        value=op["initial"]
        if not (type(value) in (int,float) and 0.01<=value<=10 or isinstance(value,list) and len(value)==3 and all(type(v) in (int,float) and -20<=v<=20 for v in value)):
            raise ValueError("invalid bounded initial Taper distance")
        if isinstance(value,list):
            a,b=axis(op)
            direction=[b[i]-a[i] for i in range(3)]
            length=math.sqrt(sum(x*x for x in direction))
            direction=[x/length for x in direction]
            delta=[value[i]-a[i] for i in range(3)]
            z=sum(delta[i]*direction[i] for i in range(3))
            if math.sqrt(sum((delta[i]-z*direction[i])**2 for i in range(3)))<0.01:
                raise ValueError("initial Taper point must be away from the axis")
        names.add(op["id"])


def diagnostic_request():
    cases=[]
    base=dict(op="taper_preview",shape="Points",copy=False,rigid=False,flat=False,infinite=False,preserve=False,
              initial=2.,phase="End",view="Front",display_mode="Wireframe",cursor="Valid",finish="Click")
    for change in [{},dict(axis="Tilt",view="Top"),dict(axis="Tilt",view="Top",spine_z=3.),dict(axis="Spatial",view="Perspective"),
                   dict(shape="Line",phase="Start",finish="Cancel"),dict(shape="Line",cursor="Degenerate",finish="Cancel")]:
        cases.append(dict(base,id="Taper_diagnostic_"+str(len(cases)),**change))
    return dict(protocol_version=1,iterations=1,operations=cases)


def request():
    base=dict(op="taper_preview",copy=False,rigid=False,flat=False,infinite=False,preserve=False,
              initial=2.,phase="End",view="Front",display_mode="Wireframe",cursor="Valid",finish="Click")
    changes=[dict(shape=s) for s in ("Points","Line","Curve","Surface","Box","Mesh")]
    changes += [dict(shape="Line",**c) for c in (dict(flat=True),dict(infinite=True),dict(preserve=True),dict(copy=True),dict(copy=True,phase="Repeat"),dict(finish="Cancel"),dict(phase="Start",finish="Cancel"),dict(cursor="Degenerate",finish="Cancel"),dict(flat=True,initial=[0.,2.,0.]))]
    changes += [dict(shape=s,rigid=True) for s in ("Points","Box")]
    changes += [dict(shape=s,preserve=True) for s in ("Curve","Surface")]
    changes += [dict(shape=s,display_mode=m) for s in ("Surface","Box","Mesh") for m in ("Shaded","Ghosted")]
    changes += [dict(shape="Points",axis="Tilt",view="Top"),dict(shape="Points",axis="Tilt",view="Top",spine_z=3.),dict(shape="Points",axis="Spatial",view="Perspective"),dict(shape="Line",axis="Spatial",view="Perspective",flat=True),dict(shape="Line",axis="Tilt",view="Top",phase="Start",finish="Cancel")]
    operations=[dict(base,id="Taper_preview_"+str(i),**change) for i,change in enumerate(changes)]
    result=dict(protocol_version=1,iterations=1,operations=operations)
    validate_request(result)
    return result


def recipe(op):
    a,b=axis(op)
    def token(v):
        # Distances and coordinates are bounded and encoded using IEEE integers.
        from number_token import number_token
        return "w"+",".join(number_token(x) for x in v) if isinstance(v,list) else number_token(v)
    options=" ".join("_"+name+"=_"+("Yes" if op[key] else "No") for name,key in FLAGS if name!="PreserveStructure" or op["shape"]!="Box")
    prefix="_Taper "+token(a)+" "+token(b)+" "+options+" "
    if op["phase"]!="Start":prefix+=token(op["initial"])+" "
    if op["phase"]=="Repeat":prefix+="1 "
    return dict(prefix=prefix,aim=[b[0]+1.,b[1],b[2]] if op.get("axis","Z")=="Spatial" else [1.,1. if op["view"]=="Top" else 0.,b[2]])


def radius(value,start,end,host):
    if not isinstance(value,list):return float(value)
    v=host["_point"](value)-host["_point"](start)
    normal=host["_point"](end)-host["_point"](start)
    normal.Unitize()
    return (v-normal*(v*normal)).Length

def run(op, host):
    validate_request(dict(protocol_version=1, operations=[op]))
    Rhino, System = host["Rhino"], host["System"]
    doc = Rhino.RhinoDoc.ActiveDoc
    if list(doc.Objects):
        raise ValueError("Taper preview requires an empty owned document")
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
    captured = os.path.join(root, "taper-preview-captured-" + op["id"] + ".json")
    ready = os.path.join(root, "taper-preview-ready-" + op["id"] + ".json")
    pending, errors, sources, active, witness, arrived = [], [], [], [], [], []
    timer = Timer()
    timer.Interval = 100
    hooks_owned = False

    class Mouse(Rhino.UI.MouseCallback):
        def OnEndMouseMove(self, event):
            if (event.View.ActiveViewport.Id == vp.Id
                    and [int(event.ViewportPoint.X), int(event.ViewportPoint.Y)] == [x, y]):
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
        if event.CommandEnglishName == "Taper":
            active.append(True)
            send("Taper preview " + op["id"] + ": command begun")

    def ended(sender, event):
        if event.CommandEnglishName == "Taper":
            active[:] = []
            send("Taper preview " + op["id"] + ": command ended " + str(event.CommandResult))

    def tick(sender, event):
        if pending or errors or not active or not arrived or not Rhino.Input.RhinoGet.InGetPoint(doc):
            return
        try:
            send("Taper preview " + op["id"] + ": capture pending")
            ok, ray = vp.GetFrustumLine(x, y)
            if not ok:
                raise ValueError("Taper preview ray unavailable")
            frame = capture(vp, host["_point"](aim), [x, y], host)
            frame["ray"] = [host["_xyz"](ray.From), host["_xyz"](ray.To)]
            pending.append(dict(objects=snapshot(), frame=frame,
                                camera=camera_snapshot(vp, Rhino),
                                prompt=Rhino.RhinoApp.CommandPrompt))
            with open(ready + ".tmp", "w") as stream:
                json.dump(op["id"], stream)
            os.rename(ready + ".tmp", ready)
            send("Taper preview " + op["id"] + ": ready")
            timer.Stop()
        except Exception as error:
            errors.append(str(error))
            send("PICK_ABORT taper-preview-" + op["id"])

    try:
        doc.ModelAbsoluteTolerance = 1e-5
        if not vp.SetProjection(getattr(Rhino.Display.DefinedViewportProjection, op["view"]),
                                "Owned Taper preview", False):
            raise ValueError("Taper preview projection failed")
        vp.SetConstructionPlane(Rhino.Geometry.Plane.WorldXY if op["view"] == "Top" else Rhino.Geometry.Plane.WorldZX)
        vp.DisplayMode = Rhino.Display.DisplayModeDescription.FindByName(op["display_mode"])
        if not vp.ZoomBoundingBox(Rhino.Geometry.BoundingBox(host["_point"]([-5, -5, -5]),
                                                            host["_point"]([16, 8, 16]))):
            raise ValueError("Taper preview zoom failed")
        start, end = axis(op)
        owned = source_geometry(op["shape"], host)
        for geom in owned:
            if not geom.Transform(Rhino.Geometry.Transform.Translation(0, 0, op.get("spine_z", 0.0))):
                raise ValueError("Taper preview source translation failed")
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
            raise ValueError("Taper preview source insertion failed")
        if op["rigid"] and op["shape"] == "Points":
            doc.Groups.Add("OwnedTaperPreview", sources)
        if op["cursor"] == "Degenerate":
            witness.append(doc.Objects.AddPoint(host["_point"](end)))
            if witness[0] == System.Guid.Empty:
                raise ValueError("Taper preview witness insertion failed")
        for key in sources:
            doc.Objects.Select(key)
        before = snapshot()
        spec = recipe(op)
        aim = end if witness else spec["aim"]
        pixel = vp.WorldToClient(host["_point"](aim))
        x, y = int(pixel.X), int(pixel.Y)
        if not 1 <= x < vp.Size.Width - 1 or not 1 <= y < vp.Size.Height - 1:
            raise ValueError("Taper cursor outside owned viewport")
        screen = view.ClientToScreen(System.Drawing.Point(x, y))
        corner = view.ClientToScreen(System.Drawing.Point(0, 0))
        valid = vp.WorldToClient(host["_point"](spec["aim"]))
        valid_screen = view.ClientToScreen(System.Drawing.Point(int(valid.X), int(valid.Y)))
        ok, valid_ray = vp.GetFrustumLine(int(valid.X), int(valid.Y))
        if not ok:
            raise ValueError("Taper valid cursor ray unavailable")
        def intersection(origin, normal):
            direction = valid_ray.Direction
            denominator = normal * direction
            if abs(denominator) <= 1e-12:
                return None
            return host["_xyz"](valid_ray.PointAt((normal * (origin - valid_ray.From)) / denominator))
        axis_vector = host["_point"](end) - host["_point"](start)
        candidates = dict(
            StartCPlane=intersection(host["_point"](start),vp.ConstructionPlane().Normal),
            EndCPlane=intersection(host["_point"](end),vp.ConstructionPlane().Normal),
            EndNormal=intersection(host["_point"](end),axis_vector),
        )
        # Accepted point-command outputs in the retained tilted and spatial
        # diagnostics distinguish this axis-normal plane from both CPlane
        # candidates. Edge-on views fall back to the CPlane through the axis.
        origin = host["_point"](start if op["phase"] == "Start" else end)
        valid_point = host["_point"](intersection(origin,axis_vector) or intersection(origin,vp.ConstructionPlane().Normal))
        metadata = dict(rect=[int(corner.X), int(corner.Y),
                              int(corner.X + vp.Size.Width), int(corner.Y + vp.Size.Height)],
                        aim_client=[float(pixel.X), float(pixel.Y)],
                        valid_screen=[int(valid_screen.X), int(valid_screen.Y)],
                        valid_point=host["_xyz"](valid_point), candidates=candidates, regions={})
        with open(os.path.join(root, "taper-preview-" + op["id"] + ".json"), "w") as stream:
            json.dump(metadata, stream)
        history = Rhino.RhinoApp.CommandHistoryWindowText
        sdk_preview, sdk_cubic_preview = [], []
        if op["phase"] != "Start" and not op["rigid"]:
            args = [host["_point"](start), host["_point"](end), radius(op["initial"],start,end,host), radius(host["_xyz"](valid_point),start,end,host), op["flat"],op["infinite"]]
            axis_normal = host["_point"](end)-host["_point"](start)
            sdk_plane=Rhino.Geometry.Plane(host["_point"](start),axis_normal)
            if isinstance(op["initial"],list):
                delta=host["_point"](op["initial"])-host["_point"](start)
                normal=Rhino.Geometry.Vector3d(axis_normal);normal.Unitize()
                x_direction=delta-normal*(delta*normal)
            else:
                x_direction=Rhino.Geometry.Vector3d.CrossProduct(axis_normal,vp.ConstructionPlane().Normal)
                if x_direction.Length <= 1e-12:
                    x_direction=vp.ConstructionPlane().XAxis
            command_plane=Rhino.Geometry.Plane(host["_point"](start),x_direction,Rhino.Geometry.Vector3d.CrossProduct(axis_normal,x_direction))
            to_sdk=Rhino.Geometry.Transform.PlaneToPlane(command_plane,sdk_plane)
            from_sdk=Rhino.Geometry.Transform.PlaneToPlane(sdk_plane,command_plane)
            morph=Rhino.Geometry.Morphs.TaperSpaceMorph(*args)
            try:
                morph.QuickPreview=True
                morph.PreserveStructure=op["preserve"]
                morph.Tolerance=1e-5
                if not morph.IsValid:
                    raise ValueError("SDK Taper preview definition invalid")
                for key in sources:
                    source=doc.Objects.FindId(key).Geometry
                    duplicate=source.ToNurbsCurve() if isinstance(source,Rhino.Geometry.Curve) else source.Duplicate()
                    try:
                        if op["flat"]:duplicate.Transform(to_sdk)
                        if not morph.Morph(duplicate):raise ValueError("SDK Taper quick morph failed")
                        if op["flat"]:duplicate.Transform(from_sdk)
                        sdk_preview.append(geometry_record(duplicate,host))
                    finally:duplicate.Dispose()
                    if isinstance(source,Rhino.Geometry.Curve):
                        duplicate=source.ToNurbsCurve()
                        try:
                            if duplicate.Degree<3 and not duplicate.IncreaseDegree(3):raise ValueError("SDK Taper elevation failed")
                            if op["flat"]:duplicate.Transform(to_sdk)
                            if not morph.Morph(duplicate):raise ValueError("SDK Taper cubic morph failed")
                            if op["flat"]:duplicate.Transform(from_sdk)
                            sdk_cubic_preview.append(geometry_record(duplicate,host))
                        finally:duplicate.Dispose()
            finally:morph.Dispose()
        with environment(dict(persistent_snaps=["Point"] if witness else []), host):
            hooks_owned = True
            with _input_hooks(timer, Mouse(), [(Rhino.Commands.Command.BeginCommand, begun),
                                           (Rhino.Commands.Command.EndCommand, ended)]):
                doc.Views.Redraw()
                send("PICK @taper-preview:" + op["id"] + " %d %d" % (screen.X, screen.Y))
                success = bool(Rhino.RhinoApp.RunScript(spec["prefix"] + "_Pause"
                                                      + (" _Enter" if op["copy"] else ""), True))
        if errors or len(pending) != 1:
            raise ValueError("Taper preview incomplete: " + str(errors))
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
        doc.ModelAbsoluteTolerance = original_tolerance
        vp.SetViewProjection(original, False)
        vp.SetCameraTarget(target, False)
        vp.Name = name
        vp.DisplayMode = mode
        original.Dispose()
        doc.Views.Redraw()
