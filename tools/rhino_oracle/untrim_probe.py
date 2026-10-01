"""Owned public UntrimAll commands, retaining complete source/output definitions."""
import re


def validate(operation):
    if (not isinstance(operation, dict)
            or set(operation) - {"preselect", "source_layer"} != {"op", "id", "sources", "keep_trim_objects"}
            or operation.get("op") != "untrim_all_command"
            or not isinstance(operation["id"], str)
            or re.match(r"^[A-Za-z0-9_.-]{1,100}\Z", operation["id"]) is None
            or type(operation["keep_trim_objects"]) is not bool
            or type(operation.get("preselect", False)) is not bool
            or type(operation.get("source_layer", False)) is not bool
            or not isinstance(operation["sources"], list)
            or not 1 <= len(operation["sources"]) <= 8):
        raise ValueError("invalid UntrimAll fixture")
    if any(not isinstance(source, dict) or source.get("type") not in
           ("point", "surface", "brep", "box") for source in operation["sources"]):
        raise ValueError("invalid UntrimAll source type")


def run(operation, tolerance, host):
    from join_probe import observe_command
    validate(operation)
    Rhino, System = host["Rhino"], host["System"]
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
        if isinstance(g, Rhino.Geometry.Surface):
            return dict(type="surface", definition=host["_nurbs_surface_definition"](g))
        if isinstance(g, Rhino.Geometry.Curve):
            return dict(type="curve", definition=host["_nurbs_curve_definition"](g))
        if isinstance(g, Rhino.Geometry.Point):
            return dict(type="point", point=host["_xyz"](g.Location))
        raise ValueError("unexpected UntrimAll geometry")
    def snapshot():
        result = []
        for obj in sorted((obj for obj in objects() if obj.Id not in original), key=lambda obj: obj.RuntimeSerialNumber):
            attrs = obj.Attributes
            result.append(dict(source=ids.index(obj.Id) if obj.Id in ids else None,
                selected=bool(obj.IsSelected(False)), name=attrs.Name,
                color=[int(attrs.ObjectColor.R), int(attrs.ObjectColor.G), int(attrs.ObjectColor.B)],
                color_source=str(attrs.ColorSource),
                current_layer=attrs.LayerIndex == document.Layers.CurrentLayerIndex,
                groups=sorted(groups.index(g) for g in (attrs.GetGroupList() or []) if g in groups),
                geometry=geometry(obj.Geometry)))
        return result
    try:
        document.Objects.UnselectAll()
        if operation.get("source_layer", False):
            layer = Rhino.DocObjects.Layer()
            try:
                layer.Name = "Viboceros untrim " + str(System.Guid.NewGuid())
                index = document.Layers.Add(layer)
                if index < 0: raise ValueError("UntrimAll source layer creation failed")
                layers.append(index)
            finally: layer.Dispose()
        constructed = []
        for index, source in enumerate(operation["sources"]):
            if source.get("type") == "box":
                g = Rhino.Geometry.Brep.CreateFromBox(Rhino.Geometry.BoundingBox(
                    Rhino.Geometry.Point3d(0, 0, 0), Rhino.Geometry.Point3d(10, 10, 10)))
            else:
                g = host["_object_source"](source, tolerance)
            owned.append(g)
            constructed.append(geometry(g))
            attrs = Rhino.DocObjects.ObjectAttributes()
            try:
                attrs.Name = "source-%d" % index
                attrs.ObjectColor = System.Drawing.Color.FromArgb(10 + index, 30, 50)
                attrs.ColorSource = Rhino.DocObjects.ObjectColorSource.ColorFromObject
                if layers: attrs.LayerIndex = layers[0]
                if isinstance(g, Rhino.Geometry.Brep):
                    key = document.Objects.AddBrep(g, attrs, None, False, False)
                elif isinstance(g, Rhino.Geometry.Surface):
                    natural = g.ToBrep()
                    try:
                        key = document.Objects.AddBrep(natural, attrs, None, False, False)
                    finally: natural.Dispose()
                else:
                    key = document.Objects.Add(g, attrs)
            finally: attrs.Dispose()
            if key == System.Guid.Empty: raise ValueError("UntrimAll source insertion failed")
            ids.append(key)
        for key in ids:
            group = document.Groups.Add("Viboceros untrim " + str(System.Guid.NewGuid()), [key])
            if group < 0: raise ValueError("UntrimAll group creation failed")
            groups.append(group)
        if operation.get("preselect", False):
            for key in ids: document.Objects.Select(key)
            # Enter also ends a selection prompt if all preselected sources
            # were rejected. On accepted sources the option already finishes
            # the command; the extra Enter is outside our EndCommand capture.
            suffix = " _Enter"
        else:
            suffix = " " + " ".join("_SelID %s" % key for key in ids) + " _Enter"
        before = snapshot()
        # Command-first options belong before selection's terminating Enter;
        # preselection instead answers the command's immediate option prompt.
        macro = "_UntrimAll _KeepTrimObjects=%s%s" % (
            "Yes" if operation["keep_trim_objects"] else "No", suffix)
        marker = "Viboceros UntrimAll " + str(System.Guid.NewGuid())
        Rhino.RhinoApp.WriteLine(marker)
        succeeded, after, events = observe_command(Rhino.Commands.Command, "UntrimAll",
            lambda: Rhino.RhinoApp.RunScript(macro, True), snapshot, lambda: [], True)
        history = Rhino.RhinoApp.CommandHistoryWindowText.split(marker, 1)
        if len(history) != 2: raise ValueError("UntrimAll history marker missing")
        for event in events: event.pop("objects", None)
        return dict(constructed=constructed, before=before, after=after,
                    succeeded=succeeded, events=events, history=history[1].strip()), 0
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
        if errors: raise ValueError("UntrimAll cleanup failed: " + str(errors))
