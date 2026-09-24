# -*- coding: utf-8 -*-
"""Unconstrained GetPoint: public 3D target, snap kind and source ownership."""
import math
import os
import re

if __package__:
    from . import snap_environment, viewport_capture
else:
    import snap_environment
    import viewport_capture


def finite(value):
    try:
        return type(value) in (int, float) and not math.isnan(value) and not math.isinf(value)
    except OverflowError:
        return False


def point(value):
    return isinstance(value, list) and len(value) == 3 and all(finite(v) for v in value)


def validate(operation):
    required = set(("op", "id", "sources", "view", "bounds", "aim", "offset", "persistent_snaps", "snap_to_meshes"))
    if (not isinstance(operation, dict) or set(operation)-set(("capture_radius","pick_diagnostics","input_settle_ms")) != required
            or operation["op"] != "point_snap"):
        raise ValueError("invalid point snap fields")
    radius = operation.get("capture_radius",12)
    if type(radius) is not int or not 1 <= radius <= 64: raise ValueError("invalid snap aperture")
    if type(operation.get("pick_diagnostics",False)) is not bool: raise ValueError("invalid picking diagnostic switch")
    settle = operation.get("input_settle_ms",0)
    if type(settle) is not int or not 0 <= settle <= 1000: raise ValueError("invalid point input settling interval")
    name = operation["id"]
    if not isinstance(name, (str, type(u""))) or re.match(r"^[A-Za-z0-9_.-]{1,100}\Z", name) is None:
        raise ValueError("invalid point snap id")
    if operation["view"] not in ("Top", "Perspective", "Front", "Right"):
        raise ValueError("unsupported point snap view")
    bounds = operation["bounds"]
    if (not isinstance(bounds, list) or len(bounds) != 2 or not all(point(p) for p in bounds)
            or any(a > b for a, b in zip(*bounds)) or bounds[0] == bounds[1]):
        raise ValueError("invalid point snap view bounds")
    offset = operation["offset"]
    if (not point(operation["aim"]) or not isinstance(offset, list) or len(offset) != 2 or
            any(type(v) is not int or not -32 <= v <= 32 for v in offset)):
        raise ValueError("invalid point snap aim or pixel offset")
    modes = operation["persistent_snaps"]
    if (not isinstance(modes, list) or len(modes) > 8 or
            any(m not in ("Point", "End", "Mid", "Cen", "Quad", "Near", "Vertex", "Int") for m in modes) or
            len(set(modes)) != len(modes) or type(operation["snap_to_meshes"]) is not bool):
        raise ValueError("invalid point snap policy")
    sources = operation["sources"]
    if not isinstance(sources, list) or not 1 <= len(sources) <= 16:
        raise ValueError("point snaps require 1 to 16 sources")
    for source in sources:
        if not isinstance(source, dict): raise ValueError("invalid point snap source")
        if source.get("type") == "line":
            if set(source) != set(("type", "start", "end")) or not point(source["start"]) or not point(source["end"]):
                raise ValueError("invalid point snap line")
        elif source.get("type") == "mesh":
            if set(source) != set(("type", "vertices", "faces")): raise ValueError("invalid point snap mesh fields")
            vertices, faces = source["vertices"], source["faces"]
            if (not isinstance(vertices, list) or not 3 <= len(vertices) <= 4096 or not all(point(v) for v in vertices)
                    or not isinstance(faces, list) or not 1 <= len(faces) <= 8192):
                raise ValueError("invalid point snap mesh")
            for face in faces:
                if (not isinstance(face, list) or len(face) not in (3,4) or
                        any(type(i) is not int or not 0 <= i < len(vertices) for i in face) or len(set(face)) != len(face)):
                    raise ValueError("invalid point snap mesh face")
        else:
            raise ValueError("point snap diagnostic supports only line and mesh sources")


def validate_request(request):
    if (not isinstance(request,dict) or type(request.get("protocol_version")) is not int or request["protocol_version"] != 1 or
            type(request.get("iterations", 1)) is not int or request.get("iterations", 1) != 1 or
            not isinstance(request.get("operations"), list) or not 1 <= len(request["operations"]) <= 128):
        raise ValueError("point snaps require protocol 1, one iteration and 1 to 128 dedicated operations")
    names = set()
    for operation in request["operations"]:
        validate(operation)
        if operation["id"] in names: raise ValueError("duplicate point snap id")
        names.add(operation["id"])


def pick(operation, host):
    import clr
    clr.AddReference("System.Windows.Forms")
    from System.Windows.Forms import Timer
    Rhino, System = host["Rhino"], host["System"]
    document = Rhino.RhinoDoc.ActiveDoc
    view = document.Views.ActiveView
    timer, getter = Timer(), Rhino.Input.Custom.GetPoint()
    timer.Interval = 100
    getter.SetCommandPrompt("Viboceros owned snap calibration")
    frames, errors = [], []
    progress = os.path.join(os.path.dirname(os.path.abspath(host["__file__"])), "worker-progress.log")
    def send(line):
        with open(progress, "a") as stream:
            stream.write(line + "\n")
            stream.flush()
    def tick(sender, event):
        if frames or errors or not document.InGetPoint: return
        try:
            viewport = view.ActiveViewport
            aim = host["_point"](operation["aim"])
            pixel = viewport.WorldToClient(aim)
            x, y = int(pixel.X)+operation["offset"][0], int(pixel.Y)+operation["offset"][1]
            if not 1 <= x < viewport.Size.Width-1 or not 1 <= y < viewport.Size.Height-1:
                raise ValueError("point snap click outside owned viewport")
            frame = viewport_capture.capture(viewport, aim, [x,y], host)
            screen = view.ClientToScreen(System.Drawing.Point(x,y))
            send("PICK @point:%s %d %d" % (operation["id"], screen.X, screen.Y))
            frames.append(frame)
        except Exception as error:
            errors.append(str(error))
            timer.Stop()
            try:
                send("PICK_ABORT " + operation["id"])
            except Exception as abort_error:
                errors.append("cannot request owned cancellation: " + str(abort_error))
    timer.Tick += tick
    try:
        timer.Start()
        result = getter.Get()
        if errors or len(frames) != 1 or result != Rhino.Input.GetResult.Point:
            raise ValueError("point snap input failed: %s %s" % (result, errors))
        actual_point = host["_xyz"](getter.Point())
        if not point(actual_point): raise ValueError("nonfinite picked point")
        reference = getter.PointOnObject()
        try:
            component = None if reference is None else dict(
                type=str(reference.GeometryComponentIndex.ComponentIndexType),
                index=int(reference.GeometryComponentIndex.Index))
            return dict(point=actual_point, kind=str(getter.OsnapEventType),
                        component=component, frame=frames[0]), None if reference is None else reference.ObjectId
        finally:
            if reference is not None: reference.Dispose()
    finally:
        timer.Stop()
        timer.Tick -= tick
        timer.Dispose()
        getter.Dispose()


def topology_wires(geometry, host):
    """Public topology indices/lines, not an inferred face-order mapping."""
    if not isinstance(geometry, host["Rhino"].Geometry.Mesh): return None
    edges, wires = geometry.TopologyEdges, []
    for index in range(edges.Count):
        line = edges.EdgeLine(index)
        ends = [host["_xyz"](line.From),host["_xyz"](line.To)]
        if not all(point(end) for end in ends): raise ValueError("nonfinite mesh topology wire")
        wires.append(ends)
    return wires


def wire_pick_diagnostics(view, result, radius, host, geometries=()):
    """Read-only public PickContext queries, distinct from actual GetPoint snaps."""
    Rhino, System = host["Rhino"],host["System"]
    context = Rhino.Input.Custom.PickContext()
    try:
        x,y = result["frame"]["click_client"]
        viewport = view.ActiveViewport
        success,line = viewport.GetFrustumLine(x,y)
        if not success: raise ValueError("missing diagnostic pick ray")
        context.View = view
        context.PickStyle = Rhino.Input.Custom.PickStyle.PointPick
        context.PickLine = line
        transform = viewport.GetPickTransform(System.Drawing.Rectangle(x-radius,y-radius,2*radius,2*radius))
        context.SetPickTransform(transform)
        context.UpdateClippingPlanes()
        sources = []
        for wires in result["topology_wires"]:
            if wires is None:
                sources.append(None)
                continue
            values = []
            for a,b in wires:
                wire = Rhino.Geometry.Line(host["_point"](a),host["_point"](b))
                hit,t,depth,distance = context.PickFrustumTest(wire)
                if hit and not all(finite(v) for v in (t,depth,distance)):
                    raise ValueError("nonfinite diagnostic pick")
                values.append(dict(t=t,depth=depth,distance=distance) if hit else None)
            sources.append(values)
        meshes = []
        for geometry in geometries:
            if not isinstance(geometry,Rhino.Geometry.Mesh):
                meshes.append(None)
                continue
            picked = context.PickFrustumTest(geometry,Rhino.Input.Custom.PickContext.MeshPickStyle.WireframePicking)
            if picked[0]:
                target = host["_xyz"](picked[1])
                depth,distance,flag,index = picked[-4:]
                if not point(target) or not all(finite(v) for v in (depth,distance)):
                    raise ValueError("nonfinite diagnostic mesh pick")
                meshes.append(dict(point=target,depth=depth,distance=distance,flag=str(flag),index=int(index)))
            else:
                meshes.append(None)
        return dict(transform=[[float(transform[i,j]) for j in range(4)] for i in range(4)],sources=sources,meshes=meshes)
    finally:
        context.Dispose()


def run(operation, tolerance, host):
    validate(operation)
    Rhino, System = host["Rhino"], host["System"]
    document = Rhino.RhinoDoc.ActiveDoc
    settings = Rhino.DocObjects.ObjectEnumeratorSettings()
    settings.NormalObjects = settings.LockedObjects = settings.HiddenObjects = True
    def objects(): return list(document.Objects.GetObjectList(settings))
    # Foreign geometry could compete for snaps, even when not selected.
    if objects(): raise ValueError("point snap calibration requires an empty owned document")
    view, viewport = document.Views.ActiveView, document.Views.ActiveView.ActiveViewport
    original_projection = Rhino.DocObjects.ViewportInfo(viewport)
    original_name, original_target = viewport.Name, viewport.CameraTarget
    ids, owned = [], []
    def record(geometry):
        if isinstance(geometry, Rhino.Geometry.Mesh): return dict(mesh=host["_polygon_mesh_value"](geometry))
        return dict(line=[host["_xyz"](geometry.PointAtStart), host["_xyz"](geometry.PointAtEnd)])
    try:
        for source in operation["sources"]:
            geometry = host["_object_source"](source, tolerance)
            if geometry is None: raise ValueError("missing point snap geometry")
            owned.append(geometry)
            if not geometry.IsValid: raise ValueError("invalid point snap geometry")
            key = document.Objects.AddMesh(geometry) if isinstance(geometry, Rhino.Geometry.Mesh) else document.Objects.AddCurve(geometry)
            if key == System.Guid.Empty: raise ValueError("point snap source insertion failed")
            ids.append(key)
            if record(document.Objects.FindId(key).Geometry) != record(geometry):
                raise ValueError("point snap insertion changed geometry")
        projection = getattr(Rhino.Display.DefinedViewportProjection, operation["view"])
        if not viewport.SetProjection(projection, "Snap probe", False): raise ValueError("point snap projection failed")
        bounds = Rhino.Geometry.BoundingBox(*[host["_point"](p) for p in operation["bounds"]])
        if not viewport.ZoomBoundingBox(bounds): raise ValueError("point snap camera fit failed")
        document.Views.Redraw()
        before = [record(document.Objects.FindId(key).Geometry) for key in ids]
        with snap_environment.environment(operation, host, operation.get("capture_radius",12)) as state:
            result, key = pick(operation, host)
        if key is not None and key not in ids: raise ValueError("snap source is outside owned geometry")
        result.update(source=None if key is None else ids.index(key), before=before,
                      after=[record(document.Objects.FindId(key).Geometry) for key in ids],
                      mesh_snap_setting=state)
        # Public topology order is diagnostic evidence, not an inferred mapping
        # from face or vertex indices. Keep one entry per owned source.
        result["topology_wires"] = [topology_wires(document.Objects.FindId(key).Geometry,host) for key in ids]
        if operation.get("pick_diagnostics",False):
            result["wire_picks"] = wire_pick_diagnostics(view,result,operation.get("capture_radius",12),host,
                                                       [document.Objects.FindId(key).Geometry for key in ids])
        return result, 0
    finally:
        # Attempt every independent restoration even if one API fails. Never
        # turn partial cleanup into a reported successful observation.
        actions = [("delete source",lambda key=key: document.Objects.Delete(key,True)) for key in ids]
        actions += [("dispose source",geometry.Dispose) for geometry in reversed(owned)]
        actions += [("restore projection",lambda: viewport.SetViewProjection(original_projection,False)),
                    ("restore target",lambda: viewport.SetCameraTarget(original_target,False)),
                    ("restore name",lambda: setattr(viewport,"Name",original_name)),
                    ("restore active view",lambda: setattr(document.Views,"ActiveView",view)),
                    ("dispose projection",original_projection.Dispose)]
        errors = []
        for label, action in actions:
            try:
                if action() is False: raise ValueError("API returned false")
            except Exception as error:
                errors.append(label + ": " + str(error))
        if errors: raise ValueError("point snap cleanup failed: " + "; ".join(errors))
