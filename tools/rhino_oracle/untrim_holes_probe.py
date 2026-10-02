"""Actual UntrimHoles command with explicit component preselection or owned mouse picks."""
import math
import os
import re


def validate(operation):
    required = {"op", "id", "sources", "all", "components", "maximum_edge_length", "keep_trim_objects", "pick"}
    if (not isinstance(operation, dict) or set(operation) - {"source_layer", "finish", "undo_after", "undo_redo", "preselect_kind", "trace_components", "window", "window_subobjects"} != required
            or operation.get("op") != "untrim_holes_command"
            or not isinstance(operation.get("id"), str)
            or re.match(r"^[A-Za-z0-9_.-]{1,100}\Z", operation["id"]) is None
            or type(operation["all"]) is not bool or type(operation["keep_trim_objects"]) is not bool
            or type(operation.get("source_layer", False)) is not bool
            or type(operation.get("undo_redo", False)) is not bool
            or type(operation.get("trace_components", False)) is not bool
            or type(operation.get("window_subobjects", False)) is not bool
            or operation["pick"] not in ("preselect", "mouse", "window")
            or ("preselect_kind" in operation and (operation["pick"] != "preselect" or operation["preselect_kind"] not in ("edge", "face")))
            or operation.get("finish", "Enter") not in ("Enter", "Cancel")
            or not isinstance(operation["sources"], list) or not 1 <= len(operation["sources"]) <= 8):
        raise ValueError("invalid hole command fixture")
    if operation["pick"] == "window":
        window = operation.get("window")
        if (operation["components"] or not isinstance(window, list) or len(window) != 2
                or any(not isinstance(point, list) or len(point) != 3
                    or any(type(value) not in (int, float) or not -1e6 <= value <= 1e6 for value in point) for point in window)):
            raise ValueError("invalid hole selection window")
    elif "window" in operation or "window_subobjects" in operation:
        raise ValueError("window requires a window pick")
    maximum = operation["maximum_edge_length"]
    try:
        finite = type(maximum) in (int, float) and not math.isnan(maximum) and not math.isinf(maximum)
    except OverflowError:
        finite = False
    if not finite or maximum < 0:
        raise ValueError("invalid maximum hole edge length")
    components = operation["components"]
    if (not isinstance(components, list) or len(components) > 64
            or any(not isinstance(pair, list) or len(pair) != 2
                or any(type(i) is not int or i < 0 for i in pair)
                or pair[0] >= len(operation["sources"]) for pair in components)):
        raise ValueError("invalid hole component selection")
    undo = operation.get("undo_after", [])
    if (not isinstance(undo, list) or any(type(i) is not int or not 1 <= i <= len(components) for i in undo)
            or undo != sorted(set(undo)) or (undo and operation["pick"] != "mouse")):
        raise ValueError("invalid internal hole Undo sequence")
    for source in operation["sources"]:
        if not isinstance(source, dict) or set(source) != {"brep"} or not isinstance(source["brep"], dict):
            raise ValueError("hole command requires explicit shared B-rep sources")
        path = source["brep"].get("artifact_path")
        if not isinstance(path, (str, type(u""))) or not path:
            raise ValueError("hole command requires an owned source artifact")


def drive(operation, points, host):
    """Deliver one click only after its Pause appears in the owned command history."""
    Rhino, System = host["Rhino"], host["System"]
    suffix = " _" + operation.get("finish", "Enter")
    script = "_UntrimHoles" + "".join(" _Pause" + (
        " _Undo" if index + 1 in operation.get("undo_after", []) else "")
        for index in range(len(points))) + suffix
    if operation["pick"] == "window": script = "_UntrimHoles"
    if not points: return Rhino.RhinoApp.RunScript(script, True)
    import clr
    clr.AddReference("System.Windows.Forms")
    from System.Windows.Forms import Timer
    timer = Timer()
    timer.Interval = 100
    start = Rhino.RhinoApp.CommandHistoryWindowText
    progress_time = [System.DateTime.UtcNow]
    sent, errors, finish_sent = [], [], []
    path = os.path.join(os.path.dirname(os.path.abspath(host["__file__"])), "worker-progress.log")
    def tick(sender, event):
        try:
            history = Rhino.RhinoApp.CommandHistoryWindowText
            if not history.startswith(start): raise ValueError("hole command history changed unexpectedly")
            if (System.DateTime.UtcNow - progress_time[0]).TotalSeconds > 15:
                raise ValueError("hole component pick was not accepted within 15 seconds")
            index = len(sent)
            if operation["pick"] == "window" and index == len(points):
                if not finish_sent and (System.DateTime.UtcNow - progress_time[0]).TotalSeconds > 1:
                    with open(path, "a") as stream:
                        stream.write("PICK @hole-finish:%s:%s 1 1\n" % (operation["id"], operation.get("finish", "Enter")))
                        stream.flush()
                    finish_sent.append(True)
                return
            if index >= len(points): return
            if operation["pick"] != "window" and history[len(start):].count("_Pause") < index + 1: return
            view = Rhino.RhinoDoc.ActiveDoc.Views.ActiveView
            viewport = view.ActiveViewport
            pixel = viewport.WorldToClient(points[index])
            x, y = int(pixel.X), int(pixel.Y)
            if not 1 <= x < viewport.Size.Width - 1 or not 1 <= y < viewport.Size.Height - 1:
                raise ValueError("hole pick lies outside the owned viewport")
            screen = view.ClientToScreen(System.Drawing.Point(x, y))
            name = "@hole:%s:%d" % (operation["id"], index)
            if operation["pick"] == "window":
                end = viewport.WorldToClient(points[1])
                if not 1 <= end.X < viewport.Size.Width - 1 or not 1 <= end.Y < viewport.Size.Height - 1:
                    raise ValueError("hole window lies outside the owned viewport")
                end = view.ClientToScreen(System.Drawing.Point(int(end.X), int(end.Y)))
                name = "@hole-window:%s:%s:%d:%d" % (operation["id"],
                    "sub" if operation.get("window_subobjects", False) else "plain", end.X, end.Y)
            with open(path, "a") as stream:
                stream.write("PICK %s %d %d\n" % (name, screen.X, screen.Y))
                stream.flush()
            sent.append(index)
            if operation["pick"] == "window": sent.append(1)
            progress_time[0] = System.DateTime.UtcNow
        except Exception as error:
            errors.append(str(error)); timer.Stop()
            with open(path, "a") as stream:
                stream.write("PICK_ABORT %s\n" % operation["id"]); stream.flush()
    timer.Tick += tick
    try:
        timer.Start()
        result = Rhino.RhinoApp.RunScript(script, True)
        if errors or len(sent) != len(points): raise ValueError("incomplete hole mouse input: " + str(errors))
        return result
    finally:
        timer.Stop(); timer.Tick -= tick; timer.Dispose()


def run(operation, tolerance, host):
    from join_probe import observe_command
    validate(operation)
    Rhino, System = host["Rhino"], host["System"]
    if operation.get("undo_redo", False) and Rhino.Commands.Command.InCommand():
        raise ValueError("hole history requires idle execution outside RunPythonScript")
    document = Rhino.RhinoDoc.ActiveDoc
    settings = Rhino.DocObjects.ObjectEnumeratorSettings()
    settings.NormalObjects = settings.HiddenObjects = settings.LockedObjects = True
    def objects(): return list(document.Objects.GetObjectList(settings))
    original = set(obj.Id for obj in objects())
    selected_before = [obj.Id for obj in objects() if obj.IsSelected(False)]
    ids, groups, owned, layers = [], [], [], []
    def geometry(g):
        if isinstance(g, Rhino.Geometry.Brep):
            return dict(type="brep", definition=host["_interchange_brep_record"](g, include_samples=False),
                        untrimmed=[bool(f.IsSurface) for f in g.Faces])
        if isinstance(g, Rhino.Geometry.Curve):
            return dict(type="curve", definition=host["_nurbs_curve_definition"](g))
        raise ValueError("unexpected hole command output")
    def snapshot():
        result = []
        for obj in sorted((obj for obj in objects() if obj.Id not in original), key=lambda obj: obj.RuntimeSerialNumber):
            attrs = obj.Attributes
            result.append(dict(source=ids.index(obj.Id) if obj.Id in ids else None,
                selected=bool(obj.IsSelected(False)), name=attrs.Name,
                color=[int(attrs.ObjectColor.R), int(attrs.ObjectColor.G), int(attrs.ObjectColor.B)],
                color_source=str(attrs.ColorSource), current_layer=attrs.LayerIndex == document.Layers.CurrentLayerIndex,
                groups=sorted(groups.index(g) for g in (attrs.GetGroupList() or []) if g in groups),
                geometry=geometry(obj.Geometry)))
        return result
    def selected_components():
        result = []
        kinds = {"BrepFace": "face", "BrepEdge": "edge"}
        for source, key in enumerate(ids):
            for component in (document.Objects.FindId(key).GetSelectedSubObjects() or []):
                kind = str(component.ComponentIndexType)
                if kind not in kinds: raise ValueError("unexpected selected component kind")
                result.append([source, kinds[kind], int(component.Index)])
        return sorted(result)
    try:
        document.Objects.UnselectAll()
        # Native preselected holes are processed immediately, before the first
        # option prompt. Establish remembered choices while no components are
        # selected, then observe only the following actual command invocation.
        options = " _All=%s _MaximumEdgeLength %.17g _KeepTrimObjects=%s" % (
            "Yes" if operation["all"] else "No", operation["maximum_edge_length"],
            "Yes" if operation["keep_trim_objects"] else "No")
        Rhino.RhinoApp.RunScript("_UntrimHoles" + options + " _Cancel", False)
        if operation.get("source_layer", False):
            layer = Rhino.DocObjects.Layer()
            try:
                layer.Name = "Viboceros holes " + str(System.Guid.NewGuid())
                index = document.Layers.Add(layer)
                if index < 0: raise ValueError("hole source layer creation failed")
                layers.append(index)
            finally: layer.Dispose()
        constructed = []
        for index, source in enumerate(operation["sources"]):
            path = source["brep"]["artifact_path"]
            if path.startswith("/"): path = "Z:" + path.replace("/", "\\")
            model = Rhino.FileIO.File3dm.Read(path)
            if model is None: raise ValueError("cannot read hole command source")
            try:
                entries = list(model.Objects)
                if len(entries) != 1: raise ValueError("hole source needs one object")
                g = entries[0].Geometry.Duplicate()
            finally: model.Dispose()
            owned.append(g)
            if not isinstance(g, Rhino.Geometry.Brep) or not g.IsValid: raise ValueError("invalid hole B-rep source")
            constructed.append(geometry(g))
            attrs = Rhino.DocObjects.ObjectAttributes()
            try:
                attrs.Name = "source-%d" % index
                attrs.ObjectColor = System.Drawing.Color.FromArgb(10 + index, 30, 50)
                attrs.ColorSource = Rhino.DocObjects.ObjectColorSource.ColorFromObject
                if layers: attrs.LayerIndex = layers[0]
                key = document.Objects.AddBrep(g, attrs, None, False, False)
            finally: attrs.Dispose()
            if key == System.Guid.Empty: raise ValueError("hole source insertion failed")
            ids.append(key)
        for key in ids:
            group = document.Groups.Add("Viboceros holes " + str(System.Guid.NewGuid()), [key])
            if group < 0: raise ValueError("hole source grouping failed")
            groups.append(group)
        points = []
        for source_index, component_index in operation["components"]:
            obj = document.Objects.FindId(ids[source_index])
            face_kind = operation.get("preselect_kind", "face" if operation["all"] else "edge") == "face"
            records = obj.Geometry.Faces if face_kind else obj.Geometry.Edges
            if component_index >= records.Count: raise ValueError("hole component outside source")
            if operation["pick"] == "preselect":
                kind = Rhino.Geometry.ComponentIndexType.BrepFace if face_kind else Rhino.Geometry.ComponentIndexType.BrepEdge
                component = Rhino.Geometry.ComponentIndex(kind, component_index)
                if obj.SelectSubObject(component, True, True, False) == 0: raise ValueError("hole component preselection failed")
            elif operation["all"]:
                face = records[component_index]
                candidates = [(face.Domain(0).ParameterAt(u), face.Domain(1).ParameterAt(v))
                    for u in (.2, .4, .6, .8) for v in (.2, .4, .6, .8)]
                interior = next(((u, v) for u, v in candidates if str(face.IsPointOnFace(u, v)) == "Interior"), None)
                if interior is None: raise ValueError("hole face has no certified interior pick")
                points.append(face.PointAt(*interior))
            else:
                edge = records[component_index]
                points.append(edge.PointAt(edge.Domain.ParameterAt(.375)))
        if operation["pick"] == "window":
            points = [Rhino.Geometry.Point3d(*point) for point in operation["window"]]
        if operation["pick"] in ("mouse", "window"):
            Rhino.RhinoApp.RunScript("_SetView _World _Top", False)
            Rhino.RhinoApp.RunScript("_Zoom _Extents", False)
            if operation["pick"] == "window": Rhino.RhinoApp.RunScript("_Zoom _Out", False)
            if operation["all"]:
                mode = Rhino.Display.DisplayModeDescription.FindByName("Shaded")
                if mode is None: raise ValueError("shaded mode unavailable for face picking")
                document.Views.ActiveView.ActiveViewport.DisplayMode = mode
            document.Views.Redraw()
        before = snapshot()
        components_before = selected_components() if operation.get("trace_components") else None
        marker = "Viboceros UntrimHoles " + str(System.Guid.NewGuid())
        Rhino.RhinoApp.WriteLine(marker)
        action = (lambda: drive(operation, points, host)) if operation["pick"] in ("mouse", "window") else (
            lambda: Rhino.RhinoApp.RunScript("_UntrimHoles _" + operation.get("finish", "Enter"), True))
        succeeded, after, events = observe_command(Rhino.Commands.Command, "UntrimHoles", action, snapshot, lambda: [], True)
        history = Rhino.RhinoApp.CommandHistoryWindowText.split(marker, 1)
        if len(history) != 2: raise ValueError("hole history marker missing")
        for event in events: event.pop("objects", None)
        result = dict(constructed=constructed, before=before, after=after,
            succeeded=succeeded, events=events, history=history[1].strip())
        if operation.get("trace_components"):
            result["component_selection"] = dict(before=components_before, after=selected_components())
        if operation.get("undo_redo", False):
            result["history_tested"] = before != after
            if result["history_tested"]:
                for command in ("Undo", "Redo"):
                    ok, state, event = observe_command(Rhino.Commands.Command, command,
                        lambda command=command: Rhino.RhinoApp.RunScript("_" + command, True), snapshot, lambda: [], True)
                    if not ok: raise ValueError("hole history command failed: " + command)
                    result[command.lower()] = snapshot()
                    for record in event: record.pop("objects", None)
                    result[command.lower() + "_events"] = event
                    if operation.get("trace_components"):
                        result["component_selection"][command.lower()] = selected_components()
        return result, 0
    finally:
        errors = []
        def cleanup(action):
            try: action()
            except Exception as error: errors.append(str(error))
        cleanup(lambda: Rhino.RhinoApp.RunScript("!", False))
        created = []
        cleanup(lambda: created.extend(obj for obj in objects() if obj.Id not in original))
        for obj in created: cleanup(lambda obj=obj: document.Objects.Delete(obj.Id, True))
        for group in groups: cleanup(lambda group=group: document.Groups.Delete(group))
        for layer in layers: cleanup(lambda layer=layer: document.Layers.Delete(layer, True))
        cleanup(document.Objects.UnselectAll)
        for key in selected_before: cleanup(lambda key=key: document.Objects.Select(key))
        for g in reversed(owned): cleanup(g.Dispose)
        if errors: raise ValueError("hole command cleanup failed: " + str(errors))
