"""Owned AreaCentroid command observations; public APIs only."""
import re


def validate(operation):
    if (not isinstance(operation,dict) or operation.get("op") != "area_centroid_command"
            or set(operation) - set(("groups", "selected", "preselect")) != set(("op", "id", "sources"))):
        raise ValueError("invalid area centroid fields")
    if not isinstance(operation["id"],str) or re.match(r"^[A-Za-z0-9_.-]{1,100}\Z",operation["id"]) is None:
        raise ValueError("invalid area centroid id")
    if type(operation.get("preselect", True)) is not bool: raise ValueError("invalid preselection")
    sources = operation["sources"]
    if not isinstance(sources, list) or not 1 <= len(sources) <= 32:
        raise ValueError("expected 1 to 32 centroid sources")
    def indices(values):
        if (not isinstance(values, list) or not values
                or any(type(i) is not int or not 0 <= i < len(sources) for i in values)
                or len(set(values)) != len(values)):
            raise ValueError("invalid centroid source indices")
    groups = operation.get("groups", [])
    if not isinstance(groups, list) or len(groups) > 16:
        raise ValueError("invalid centroid groups")
    for group in groups: indices(group)
    indices(operation.get("selected", list(range(len(sources)))))


def run(operation, tolerance, helper):
    import Rhino
    import System
    from join_probe import observe_command
    validate(operation)
    document = Rhino.RhinoDoc.ActiveDoc
    settings = Rhino.DocObjects.ObjectEnumeratorSettings()
    settings.NormalObjects = settings.HiddenObjects = settings.LockedObjects = True
    def objects(): return list(document.Objects.GetObjectList(settings))
    before = set(obj.Id for obj in objects())
    selected_before = [obj.Id for obj in objects() if obj.IsSelected(False)]
    groups_before = set(i for i in range(document.Groups.Count) if not document.Groups.IsDeleted(i))
    geometries, ids, original, properties, constructed, insertions, tight = [], [], [], [], [], [], []
    def geometry_record(geometry):
        if isinstance(geometry, Rhino.Geometry.Mesh):
            return helper["_polygon_mesh_value"](geometry)
        if isinstance(geometry, Rhino.Geometry.Brep): return helper["_interchange_brep_record"](geometry)
        if isinstance(geometry, Rhino.Geometry.Surface): return helper["_nurbs_surface_definition"](geometry)
        if isinstance(geometry, Rhino.Geometry.Curve): return helper["_nurbs_curve_definition"](geometry)
        raise ValueError("unsupported centroid source")
    try:
        document.Objects.UnselectAll()
        for source in operation["sources"]:
            geometry = helper["_object_source"](source, tolerance)
            geometries.append(geometry)
            key = document.Objects.Add(geometry)
            if key == System.Guid.Empty: raise ValueError("centroid source insertion failed")
            ids.append(key)
            stored = document.Objects.FindId(key).Geometry
            promoted = geometry.ToBrep() if isinstance(geometry, Rhino.Geometry.Surface) else None
            try:
                expected = geometry_record(promoted if promoted is not None else geometry)
                if geometry_record(stored) != expected:
                    if not isinstance(geometry, Rhino.Geometry.Brep):
                        raise ValueError("centroid source changed during document insertion")
                    reversed_copy = geometry.DuplicateBrep()
                    try:
                        reversed_copy.Flip()
                        if geometry_record(stored) != geometry_record(reversed_copy):
                            raise ValueError("centroid insertion change is not a complete B-rep reversal")
                    finally: reversed_copy.Dispose()
                    insertions.append("reversed")
                else: insertions.append("unchanged")
                constructed.append(expected)
            finally:
                if promoted is not None: promoted.Dispose()
            original.append(geometry_record(stored))
            mass = Rhino.Geometry.AreaMassProperties.Compute(stored)
            try:
                properties.append(None if mass is None else dict(area=float(mass.Area), centroid=helper["_xyz"](mass.Centroid)))
            finally:
                if mass is not None: mass.Dispose()
            if isinstance(stored, (Rhino.Geometry.Brep, Rhino.Geometry.Surface)):
                # The explicit-tolerance overload is defined for Breps. Rhino
                # normally promotes document surfaces on insertion already.
                promoted = stored.ToBrep() if isinstance(stored, Rhino.Geometry.Surface) else None
                mass = None
                try:
                    mass = Rhino.Geometry.AreaMassProperties.Compute(promoted if promoted is not None else stored,
                        True, True, False, False, tolerance["relative"], tolerance["absolute"])
                    tight.append(None if mass is None else dict(area=float(mass.Area), centroid=helper["_xyz"](mass.Centroid)))
                finally:
                    if mass is not None: mass.Dispose()
                    if promoted is not None: promoted.Dispose()
            else: tight.append(properties[-1])
        for group in operation.get("groups", []):
            if document.Groups.Add("Viboceros centroid " + str(System.Guid.NewGuid()), [ids[i] for i in group]) < 0:
                raise ValueError("centroid group insertion failed")
        selected_indices = operation.get("selected", list(range(len(ids))))
        if operation.get("preselect", True):
            for i in selected_indices: document.Objects.Select(ids[i])
            macro = "_AreaCentroid _Enter"
        else:
            macro = "_AreaCentroid " + " ".join("_SelID %s" % ids[i] for i in selected_indices) + " _Enter"
        marker = "Viboceros centroid " + str(System.Guid.NewGuid())
        Rhino.RhinoApp.WriteLine(marker)
        succeeded, _, _ = observe_command(Rhino.Commands.Command, "AreaCentroid",
            lambda: Rhino.RhinoApp.RunScript(macro, True), lambda: None, lambda: [])
        history = Rhino.RhinoApp.CommandHistoryWindowText.split(marker, 1)
        if len(history) != 2: raise ValueError("centroid history marker missing")
        points = []
        for obj in objects():
            if obj.Id in before or obj.Id in ids: continue
            if not isinstance(obj.Geometry, Rhino.Geometry.Point):
                raise ValueError("unexpected centroid output geometry")
            points.append(dict(point=helper["_xyz"](obj.Geometry.Location), selected=bool(obj.IsSelected(False)),
                               current_layer=obj.Attributes.LayerIndex == document.Layers.CurrentLayerIndex,
                               name=obj.Attributes.Name or "", groups=int(obj.Attributes.GroupCount)))
        selected = []
        for i, key in enumerate(ids):
            obj = document.Objects.FindId(key)
            if obj is None or geometry_record(obj.Geometry) != original[i]:
                raise ValueError("centroid changed source geometry")
            if obj.IsSelected(False): selected.append(i)
        return dict(succeeded=succeeded, properties=properties, points=points, selected=selected, inputs=original,
                    constructed_inputs=constructed, insertions=insertions, tight_properties=tight,
                    history=history[1].strip()), 0
    finally:
        Rhino.RhinoApp.RunScript("!", False)
        for obj in objects():
            if obj.Id not in before: document.Objects.Delete(obj.Id, True)
        for i in range(document.Groups.Count):
            if i not in groups_before and not document.Groups.IsDeleted(i): document.Groups.Delete(i)
        document.Objects.UnselectAll()
        for key in selected_before: document.Objects.Select(key)
        for geometry in reversed(geometries): geometry.Dispose()
