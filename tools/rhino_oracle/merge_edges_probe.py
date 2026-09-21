# -*- coding: utf-8 -*-
"""Owned MergeAllEdges/MergeEdge commands on shared topology, with undo records."""
import math
import os
import re


def mouse_command(operation):
    """Use Rhino's scriptable choice prompt after an actual component click."""
    validate(operation)
    if operation.get("op") != "merge_edge_command" or operation.get("pick") != "mouse":
        raise ValueError("edge mouse macro requires a mouse operation")
    choice = operation.get("choice", "All")
    suffix = {"Cancel": "_Cancel", "Auto": "_Enter"}.get(choice, "_" + choice)
    # Auto leaves the post-pick decision to Rhino; do not send a second Enter
    # that could repeat a command after a no-op finishes.
    return "_-MergeEdge _Pause " + suffix + ("" if choice in ("Auto", "Cancel") else " _Enter")


def validate_mouse_request(request):
    operations = request.get("operations")
    if (type(request.get("protocol_version")) is not int or request["protocol_version"] != 1 or
            type(request.get("iterations", 1)) is not int or request.get("iterations", 1) != 1 or
            not isinstance(operations, list) or not 1 <= len(operations) <= 128):
        raise ValueError("edge mouse picking requires protocol 1, one iteration and 1 to 128 cases")
    names = set()
    for operation in operations:
        if (not isinstance(operation, dict) or operation.get("op") not in ("merge_edge_command", "split_edge_command") or
                operation.get("pick") != "mouse"):
            raise ValueError("edge mouse picking requires a dedicated request")
        name = operation.get("id")
        if not isinstance(name, (str, type(u""))) or re.match(r"^[A-Za-z0-9_.-]{1,100}\Z", name) is None or name in names:
            raise ValueError("invalid edge mouse picking id")
        names.add(name)
        if operation["op"] == "split_edge_command":
            if __package__:
                from . import split_edge_probe
            else:
                import split_edge_probe
            split_edge_probe.validate(operation)
        else:
            validate(operation)


def at_idle(Rhino, callback):
    """Leave RunPythonScript's undo record before driving separate commands."""
    def idle(sender, event):
        if Rhino.Commands.Command.InCommand():
            return
        # Detach before invoking anything that can pump the message loop.
        Rhino.RhinoApp.Idle -= idle
        callback()
    Rhino.RhinoApp.Idle += idle


def validate(operation):
    sources = operation.get("sources")
    if not isinstance(sources, list) or not 1 <= len(sources) <= 32:
        raise ValueError("edge merge requires 1 to 32 source objects")
    order = operation.get("selected", list(range(len(sources))))
    if (not isinstance(order, list) or not order or
            any(type(i) is not int or not 0 <= i < len(sources) for i in order) or
            len(set(order)) != len(order)):
        raise ValueError("invalid edge merge selection")
    for key in ("preselect", "undo_redo", "cancel", "trace_commands", "object_preselect"):
        if type(operation.get(key, False)) is not bool:
            raise ValueError("edge merge options must be boolean")
    if operation.get("cancel", False) and operation.get("preselect", False):
        raise ValueError("edge merge cancellation requires command-first selection")
    selected_edge = operation.get("op") == "merge_edge_command"
    if selected_edge != ("edge" in operation):
        raise ValueError("selected edge requests require the separate merge_edge_command operation")
    if operation.get("object_preselect", False) and (not selected_edge or operation.get("pick") != "mouse"):
        raise ValueError("whole-object preselection requires an edge mouse probe")
    if "choice" in operation and (not selected_edge or operation.get("pick") != "mouse" or
            operation["choice"] not in ("Edge", "EdgeA", "EdgeB", "Both", "All", "Cancel", "Auto")):
        raise ValueError("edge merge choice requires a mouse pick and a supported choice")
    if selected_edge:
        edge = operation["edge"]
        pick = operation.get("pick", "preselect")
        if (type(edge) is not int or edge < 0 or len(order) != 1 or
                pick not in ("preselect", "point", "mouse") or
                operation.get("cancel", False) or
                (pick in ("point", "mouse") and operation.get("preselect", False)) or
                (pick == "preselect" and not operation.get("preselect", False))):
            raise ValueError("selected edge command probe requires one edge and a valid pick mode")
    for key in ("absolute_tolerance", "angular_tolerance"):
        value = operation.get(key)
        if value is not None and (type(value) not in (int, float) or
                math.isnan(value) or math.isinf(value) or value <= 0):
            raise ValueError("invalid edge merge " + key)
    for source in sources:
        if not isinstance(source, dict):
            raise ValueError("edge merge sources must be objects")
        if "brep" in source:
            brep = source["brep"]
            if not isinstance(brep, dict) or not isinstance(brep.get("artifact_path"), (str, type(u""))) or not brep["artifact_path"]:
                raise ValueError("edge merge requires shared B-rep source artifacts")
        elif source.get("type") not in ("point", "point_cloud", "mesh", "surface", "line", "nurbs",
                "arc", "circle", "ellipse", "polyline", "polycurve"):
            raise ValueError("unsupported edge merge source kind")
    if selected_edge and "brep" not in sources[order[0]] and sources[order[0]].get("type") != "surface":
        raise ValueError("selected edge command requires a surface or B-rep")
    return sources, order


def run(operation, tolerance, host):
    sources, order = validate(operation)
    return run_owned(operation, tolerance, host, sources, order)


def run_owned(operation, tolerance, host, sources, order, command_name=None, mouse_macro=None):
    """Shared owned fixture/history recording; callers validate their own command grammar."""
    import brep_join_probe
    import join_probe
    Rhino, System = host["Rhino"], host["System"]
    if operation.get("undo_redo", False) and Rhino.Commands.Command.InCommand():
        raise ValueError("edge merge history requires idle execution outside RunPythonScript")
    document = Rhino.RhinoDoc.ActiveDoc
    settings = Rhino.DocObjects.ObjectEnumeratorSettings()
    settings.NormalObjects = settings.LockedObjects = settings.HiddenObjects = True
    def objects(): return list(document.Objects.GetObjectList(settings))
    before = set(o.Id for o in objects())
    selected_before = [o.Id for o in objects() if o.IsSelected(False)]
    layer_before = document.Layers.CurrentLayerIndex
    absolute_before, angular_before = document.ModelAbsoluteTolerance, document.ModelAngleToleranceRadians
    groups_before = set(i for i in range(document.Groups.Count) if not document.Groups.IsDeleted(i))
    owned, layers, ids, groups = [], [], [], []
    history_before = Rhino.RhinoApp.CommandHistoryWindowText

    def geometry_record(geometry):
        if isinstance(geometry, Rhino.Geometry.Brep):
            return dict(brep=brep_join_probe.geometry_record(geometry, tolerance, host))
        if isinstance(geometry, Rhino.Geometry.Surface):
            brep = geometry.ToBrep()
            try: return dict(brep=brep_join_probe.geometry_record(brep, tolerance, host))
            finally: brep.Dispose()
        if isinstance(geometry, Rhino.Geometry.Point): return dict(point=host["_xyz"](geometry.Location))
        if isinstance(geometry, Rhino.Geometry.PointCloud): return dict(point_cloud=[host["_xyz"](p.Location) for p in geometry])
        if isinstance(geometry, Rhino.Geometry.Mesh): return dict(mesh=host["_polygon_mesh_value"](geometry))
        if isinstance(geometry, Rhino.Geometry.Curve): return dict(curve=host["_interchange_curve_record"](geometry))
        raise ValueError("unexpected edge merge output geometry")

    def snapshot():
        records = []
        for obj in sorted([o for o in objects() if o.Id not in before], key=lambda o: (ids.index(o.Id) if o.Id in ids else len(ids), o.RuntimeSerialNumber)):
            attributes, color = obj.Attributes, obj.Attributes.ObjectColor
            records.append(dict(source=ids.index(obj.Id) if obj.Id in ids else None,
                selected=bool(obj.IsSelected(False)), name=attributes.Name,
                layer=layers.index(attributes.LayerIndex) if attributes.LayerIndex in layers else None,
                color=[int(color.R), int(color.G), int(color.B)], color_source=str(attributes.ColorSource),
                groups=sorted(groups.index(g) for g in (attributes.GetGroupList() or []) if g in groups),
                geometry=geometry_record(obj.Geometry)))
        return records

    try:
        document.Objects.UnselectAll()
        for i, source in enumerate(sources):
            layer = Rhino.DocObjects.Layer()
            try:
                layer.Name = "Viboceros edge merge %d " % i + str(System.Guid.NewGuid())
                index = document.Layers.Add(layer)
            finally: layer.Dispose()
            if index < 0: raise ValueError("edge merge source layer failed")
            layers.append(index)
            if "brep" in source:
                path = source["brep"]["artifact_path"]
                if path.startswith("/"): path = "Z:" + path.replace("/", "\\")
                model = Rhino.FileIO.File3dm.Read(path)
                if model is None: raise ValueError("cannot read edge merge source artifact")
                try:
                    entries = list(model.Objects)
                    if len(entries) != 1: raise ValueError("edge merge artifact requires one object")
                    geometry = entries[0].Geometry.Duplicate()
                finally: model.Dispose()
                if not isinstance(geometry, Rhino.Geometry.Brep):
                    if geometry is not None: geometry.Dispose()
                    raise ValueError("edge merge shared artifact must contain a B-rep")
            else: geometry = host["_object_source"](source, tolerance)
            if geometry is None: raise ValueError("missing edge merge source")
            owned.append(geometry)
            if not geometry.IsValid: raise ValueError("invalid edge merge source")
            attributes = Rhino.DocObjects.ObjectAttributes()
            try:
                attributes.Name = "source-%d" % i
                attributes.LayerIndex = index
                attributes.ObjectColor = System.Drawing.Color.FromArgb(11+i, 22, 33)
                attributes.ColorSource = Rhino.DocObjects.ObjectColorSource.ColorFromObject
                if isinstance(geometry, Rhino.Geometry.Brep): key = document.Objects.AddBrep(geometry, attributes, None, False, False)
                elif isinstance(geometry, Rhino.Geometry.Surface): key = document.Objects.AddSurface(geometry, attributes)
                elif isinstance(geometry, Rhino.Geometry.Point): key = document.Objects.AddPoint(geometry.Location, attributes)
                elif isinstance(geometry, Rhino.Geometry.PointCloud): key = document.Objects.AddPointCloud(geometry, attributes)
                elif isinstance(geometry, Rhino.Geometry.Mesh): key = document.Objects.AddMesh(geometry, attributes)
                else: key = document.Objects.AddCurve(geometry, attributes)
            finally: attributes.Dispose()
            if key == System.Guid.Empty: raise ValueError("edge merge source insertion failed")
            ids.append(key)
            inserted, expected = geometry_record(document.Objects.FindId(key).Geometry), geometry_record(geometry)
            if inserted != expected:
                before_brep, after_brep = expected.get("brep", {}), inserted.get("brep", {})
                raise ValueError("edge merge insertion changed source geometry: face senses %s -> %s; volume %s -> %s" % (
                    before_brep.get("face_reversed"), after_brep.get("face_reversed"),
                    before_brep.get("volume"), after_brep.get("volume")))
        for members in [[key] for key in ids] + [ids]:
            index = document.Groups.Add("Viboceros edge merge " + str(System.Guid.NewGuid()), members)
            if index < 0: raise ValueError("edge merge group insertion failed")
            groups.append(index)
        if operation.get("absolute_tolerance") is not None:
            document.ModelAbsoluteTolerance = float(operation["absolute_tolerance"])
            if document.ModelAbsoluteTolerance != float(operation["absolute_tolerance"]): raise ValueError("edge merge absolute tolerance was not accepted")
        if operation.get("angular_tolerance") is not None:
            document.ModelAngleToleranceRadians = float(operation["angular_tolerance"])
            if document.ModelAngleToleranceRadians != float(operation["angular_tolerance"]): raise ValueError("edge merge angular tolerance was not accepted")
        eligible = any(isinstance(owned[i], (Rhino.Geometry.Brep, Rhino.Geometry.Surface)) for i in order)
        command = command_name or ("MergeEdge" if "edge" in operation else "MergeAllEdges")
        if operation.get("object_preselect", False):
            for i in order:
                if not document.Objects.Select(ids[i]): raise ValueError("edge merge whole-object preselection failed")
        mouse_pick = None
        if "edge" in operation:
            obj = document.Objects.FindId(ids[order[0]])
            edge = operation["edge"]
            if not isinstance(obj.Geometry, Rhino.Geometry.Brep) or edge >= obj.Geometry.Edges.Count:
                raise ValueError("selected edge command index outside source")
            if operation.get("pick", "preselect") == "preselect":
                component = Rhino.Geometry.ComponentIndex(Rhino.Geometry.ComponentIndexType.BrepEdge, edge)
                if obj.SelectSubObject(component, True, True, False) == 0:
                    raise ValueError("selected edge command preselection failed")
                script = "_MergeEdge _Enter"
            else:
                curve = obj.Geometry.Edges[edge]
                point = curve.PointAt(curve.Domain.ParameterAt(0.375))
                if operation.get("pick") == "mouse":
                    view = document.Views.ActiveView
                    viewport = view.ActiveViewport
                    if not viewport.SetProjection(Rhino.Display.DefinedViewportProjection.Perspective, "Probe", False):
                        raise ValueError("edge picking viewport setup failed")
                    Rhino.RhinoApp.RunScript("_Zoom _Extents", False)
                    document.Views.Redraw()
                    pixel = viewport.WorldToClient(point)
                    if not 1 <= pixel.X < viewport.Size.Width - 1 or not 1 <= pixel.Y < viewport.Size.Height - 1:
                        raise ValueError("edge pick lies outside the owned viewport")
                    screen = view.ClientToScreen(System.Drawing.Point(int(pixel.X), int(pixel.Y)))
                    mouse_pick = "PICK %s %d %d" % (operation["id"], screen.X, screen.Y)
                    script = mouse_macro(operation, curve, host) if mouse_macro else mouse_command(operation)
                else:
                    script = "_MergeEdge " + host["_command_point"](host["_xyz"](point)) + " _Enter"
        elif operation.get("preselect", False):
            for i in order:
                if not document.Objects.Select(ids[i]): raise ValueError("edge merge preselection failed")
            script = "_MergeAllEdges" if eligible else "_MergeAllEdges _Cancel"
        else:
            script = "_MergeAllEdges " + " ".join("_SelID %s" % ids[i] for i in order)
            script += " _Cancel" if operation.get("cancel", False) or not eligible else " _Enter"
        initial = snapshot()
        initial_serials = dict((o.Id, o.RuntimeSerialNumber) for o in objects() if o.Id not in before)
        host["_record_progress"]("edge merge command: " + script)
        trace = operation.get("trace_commands", False)
        def drive():
            if mouse_pick is not None:
                # Unlike optional diagnostics, this write requests input and
                # must fail the probe if the host cannot receive it.
                path = os.path.join(os.path.dirname(os.path.abspath(host["__file__"])), "worker-progress.log")
                with open(path, "a") as stream:
                    stream.write(mouse_pick + "\n")
                    stream.flush()
            return host["_run_surface_script"](script, True)
        succeeded, after, events = join_probe.observe_command(Rhino.Commands.Command, command,
            drive, snapshot, lambda: [], trace)
        result = dict(before=initial, after=after, succeeded=succeeded,
            absolute_tolerance=float(document.ModelAbsoluteTolerance), angular_tolerance=float(document.ModelAngleToleranceRadians))
        if trace: result["command_events"] = events
        if operation.get("undo_redo", False):
            # Object replacement is authoritative; repeated mass-property
            # integration is not a safe predicate for whether Undo is ours.
            changed = initial_serials != dict((o.Id, o.RuntimeSerialNumber) for o in objects() if o.Id not in before)
            result["history_tested"] = changed
            if changed:
                for command in ("Undo", "Redo"):
                    ok, state, events = join_probe.observe_command(Rhino.Commands.Command, command,
                        lambda: host["_run_surface_script"]("_" + command, True), snapshot, lambda: [], trace)
                    if not ok: raise ValueError("edge merge history command failed: " + command)
                    result[command.lower()] = snapshot()
                    if trace:
                        result[command.lower() + "_events"] = events
                        result[command.lower() + "_event_snapshot"] = state
                # A Success event alone is not evidence that Undo did anything.
                def topology(records):
                    return [(r["source"], len(r["geometry"]["brep"]["edges"]))
                            for r in records if "brep" in r["geometry"]]
                if topology(result["undo"]) != topology(initial) or topology(result["redo"]) != topology(after):
                    raise ValueError("edge merge history did not restore original and merged topology")
        if trace:
            history = Rhino.RhinoApp.CommandHistoryWindowText
            result["command_history"] = history[len(history_before):] if history.startswith(history_before) else history
        return result, 0
    finally:
        Rhino.RhinoApp.RunScript("_Cancel", False)
        document.ModelAbsoluteTolerance, document.ModelAngleToleranceRadians = absolute_before, angular_before
        document.Objects.UnselectAll()
        for obj in objects():
            if obj.Id not in before: document.Objects.Delete(obj.Id, True)
        for i in range(document.Groups.Count):
            if i not in groups_before and not document.Groups.IsDeleted(i): document.Groups.Delete(i)
        document.Layers.SetCurrentLayerIndex(layer_before, True)
        for i in reversed(layers): document.Layers.Delete(i, True)
        for geometry in reversed(owned): geometry.Dispose()
        for key in selected_before: document.Objects.Select(key)
