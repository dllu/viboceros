# -*- coding: utf-8 -*-
"""Owned, public Join/JoinCopy commands; no geometry/order normalization."""


def observe_command(event_source, command, run, snapshot, selected, trace=False):
    """Capture one command, not commands left over in its driving macro."""
    results, events, errors = [], [], []
    def ended(sender, event):
        try:
            name, result = event.CommandEnglishName, str(event.CommandResult)
            objects = None
            if name == command:
                objects = snapshot()
                results.append((result == "Success", objects))
            if trace:
                record = dict(name=name, result=result, selected=selected())
                if objects is not None: record["objects"] = objects
                events.append(record)
        except Exception as error:
            # Rhino event dispatch may swallow handler exceptions. Fail the
            # probe after detaching instead of publishing an incomplete record.
            errors.append(str(error))
    event_source.EndCommand += ended
    try:
        run()
    finally:
        event_source.EndCommand -= ended
    if errors: raise ValueError("join command observer failed: " + str(errors))
    if len(results) != 1:
        raise ValueError("expected exactly one completed %s command, got %d" % (command, len(results)))
    return results[0][0], results[0][1], events


def validate(operation):
    import math
    sources = operation["sources"]
    if not isinstance(sources, list) or not 1 <= len(sources) <= 32:
        raise ValueError("join requires 1 to 32 sources")
    order = operation.get("selected", list(range(len(sources))))
    if (not isinstance(order, list) or not order or
            any(type(i) is not int or i < 0 or i >= len(sources) for i in order) or
            len(set(order)) != len(order)):
        raise ValueError("invalid join selection")
    for key in ("join_disjoint", "preselect", "trace_commands"):
        if type(operation.get(key, False)) is not bool:
            raise ValueError("join options must be boolean")
    absolute = operation.get("absolute_tolerance")
    if absolute is not None and (type(absolute) not in (int, float) or math.isnan(absolute) or math.isinf(absolute) or absolute <= 0):
        raise ValueError("invalid join tolerance")
    if operation.get("command", "Join") not in ("Join", "JoinCopy"):
        raise ValueError("invalid join command")
    return sources, order


def run(operation, tolerance, host):
    sources, order = validate(operation)
    Rhino, System = host["Rhino"], host["System"]
    document = Rhino.RhinoDoc.ActiveDoc
    settings = Rhino.DocObjects.ObjectEnumeratorSettings()
    settings.NormalObjects = settings.LockedObjects = settings.HiddenObjects = True
    def objects(): return list(document.Objects.GetObjectList(settings))
    before = set(obj.Id for obj in objects())
    selected_before = [obj.Id for obj in objects() if obj.IsSelected(False)]
    layer_before = document.Layers.CurrentLayerIndex
    tolerance_before = document.ModelAbsoluteTolerance
    layers, groups, ids = [], [], []
    def record_objects():
        created = [obj for obj in objects() if obj.Id not in before]
        created.sort(key=lambda obj: obj.RuntimeSerialNumber)
        records = []
        for obj in created:
            attributes = obj.Attributes
            color = attributes.ObjectColor
            record = dict(
                source=ids.index(obj.Id) if obj.Id in ids else None,
                selected=bool(obj.IsSelected(False)), name=attributes.Name,
                layer=layers.index(attributes.LayerIndex) if attributes.LayerIndex in layers else None,
                color=[int(color.R), int(color.G), int(color.B)],
                groups=sorted(groups.index(g) for g in (attributes.GetGroupList() or []) if g in groups))
            if isinstance(obj.Geometry, Rhino.Geometry.Mesh):
                record["mesh"] = host["_polygon_mesh_value"](obj.Geometry)
            elif isinstance(obj.Geometry, Rhino.Geometry.Curve):
                record["curve"] = host["_interchange_curve_record"](obj.Geometry)
            else:
                raise ValueError("unexpected join geometry")
            records.append(record)
        return records
    def source_mesh(source):
        mesh = Rhino.Geometry.Mesh()
        try:
            mesh.Vertices.UseDoublePrecisionVertices = True
            for point in source["vertices"]:
                if mesh.Vertices.Add(host["_point"](point)) < 0:
                    raise ValueError("join double-precision source assignment failed")
            for face in source["faces"]:
                if len(face) not in (3, 4) or any(type(i) is not int or i < 0 for i in face):
                    raise ValueError("invalid join face")
                if mesh.Faces.AddFace(*face) < 0:
                    raise ValueError("join face insertion failed")
            if not mesh.IsValid:
                raise ValueError("invalid join source geometry")
            if host["_polygon_mesh_value"](mesh) != source:
                raise ValueError("join source changed in Rhino construction")
            return mesh
        except Exception:
            mesh.Dispose()
            raise
    def source_geometry(source):
        if "type" in source:
            return host["_join_close_input"](source)
        return source_mesh(source)
    def add_geometry(geometry, attributes=None):
        if isinstance(geometry, Rhino.Geometry.Mesh):
            return document.Objects.AddMesh(geometry, attributes) if attributes else document.Objects.AddMesh(geometry)
        return document.Objects.AddCurve(geometry, attributes) if attributes else document.Objects.AddCurve(geometry)
    command = operation.get("command", "Join")
    try:
        if operation.get("absolute_tolerance") is not None:
            document.ModelAbsoluteTolerance = operation["absolute_tolerance"]
        document.Objects.UnselectAll()
        for index, source in enumerate(sources):
            layer = Rhino.DocObjects.Layer()
            attributes = Rhino.DocObjects.ObjectAttributes()
            mesh = None
            try:
                layer.Name = "Viboceros join " + str(System.Guid.NewGuid())
                layer_index = document.Layers.Add(layer)
                if layer_index < 0: raise ValueError("join layer creation failed")
                layers.append(layer_index)
                attributes.LayerIndex = layer_index
                attributes.Name = "source-%d" % index
                attributes.ObjectColor = System.Drawing.Color.FromArgb(10 + index, 30, 50)
                attributes.ColorSource = Rhino.DocObjects.ObjectColorSource.ColorFromObject
                mesh = source_geometry(source)
                object_id = add_geometry(mesh, attributes)
                if object_id == System.Guid.Empty: raise ValueError("join insertion failed")
                ids.append(object_id)
                if "type" not in source and host["_polygon_mesh_value"](document.Objects.FindId(object_id).Geometry) != source:
                    raise ValueError("join source changed during document insertion")
            finally:
                if mesh is not None: mesh.Dispose()
                attributes.Dispose()
                layer.Dispose()
        for members in [[object_id] for object_id in ids] + [ids]:
            index = document.Groups.Add("Viboceros join " + str(System.Guid.NewGuid()), members)
            if index < 0: raise ValueError("join group creation failed")
            groups.append(index)
        option = "_JoinDisjointMeshes=" + ("Yes" if operation.get("join_disjoint", False) else "No")
        selectors = " ".join("_SelID %s" % ids[i] for i in order)
        if operation.get("preselect", False):
            # Preselection executes immediately. Seed its remembered option
            # using separate owned objects, never the measured sources.
            seed_before = set(obj.Id for obj in objects())
            seed = source_geometry(sources[0])
            try:
                seed_ids = [add_geometry(seed) for _ in range(2)]
                if any(i == System.Guid.Empty for i in seed_ids):
                    raise ValueError("join option seed failed")
                seed_selectors = " ".join("_SelID %s" % i for i in seed_ids)
                host["_run_surface_script"]("_-%s %s %s _Enter" % (command, option, seed_selectors), True)
            finally:
                for obj in objects():
                    if obj.Id not in seed_before: document.Objects.Delete(obj.Id, True)
                seed.Dispose()
            document.Objects.UnselectAll()
            for i in order: document.Objects.Select(ids[i])
            script = "_-%s _Enter" % command
        else:
            script = "_-%s %s %s _Enter" % (command, option, selectors)
        host["_record_progress"]("join command: " + script)
        trace = operation.get("trace_commands", False)
        succeeded, records, events = observe_command(Rhino.Commands.Command, command,
            lambda: host["_run_surface_script"](script, True), record_objects,
            lambda: [ids.index(obj.Id) if obj.Id in ids else None for obj in objects() if obj.IsSelected(False)], trace)
        result = dict(succeeded=bool(succeeded), objects=records)
        if trace: result["command_events"] = events
        if operation.get("absolute_tolerance") is not None:
            result["absolute_tolerance"] = float(document.ModelAbsoluteTolerance)
        return result, 0
    finally:
        document.ModelAbsoluteTolerance = tolerance_before
        document.Objects.UnselectAll()
        for obj in objects():
            if obj.Id not in before: document.Objects.Delete(obj.Id, True)
        for index in groups: document.Groups.Delete(index)
        document.Layers.SetCurrentLayerIndex(layer_before, True)
        for index in reversed(layers): document.Layers.Delete(index, True)
        for object_id in selected_before: document.Objects.Select(object_id)
