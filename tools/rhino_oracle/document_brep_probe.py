"""Public Add/Replace calls on one shared, uninserted native 3dm B-rep."""
import re


def validate(operation, iterations=1):
    if type(iterations) is not int or iterations != 1:
        raise ValueError("document B-rep admission requires one iteration")
    if (not isinstance(operation, dict) or operation.get("op") != "document_brep"
            or set(operation) - {"insertion", "selected", "flip_faces", "artifact_path"} != {"op", "id", "sources"}
            or not isinstance(operation["id"], str)
            or re.match(r"^[A-Za-z0-9_.-]{1,100}\Z", operation["id"]) is None):
        raise ValueError("invalid document B-rep admission fields")
    if operation.get("insertion", "generic") not in ("generic", "no_kink"):
        raise ValueError("invalid insertion overload")
    if type(operation.get("selected", False)) is not bool:
        raise ValueError("selected must be boolean")
    sources = operation["sources"]
    if not isinstance(sources, list) or not 1 <= len(sources) <= 8 or any(not isinstance(s, dict) for s in sources):
        raise ValueError("admission requires one to eight B-rep sources")
    faces = operation.get("flip_faces", [])
    if (not isinstance(faces, list) or any(type(i) is not int or i < 0 for i in faces)
            or len(set(faces)) != len(faces)):
        raise ValueError("invalid individual face reversals")
    if "artifact_path" in operation and (not isinstance(operation["artifact_path"], str) or not operation["artifact_path"]):
        raise ValueError("invalid shared artifact path")


def geometry_record(brep, host):
    import Rhino
    return dict(orientation=str(brep.SolidOrientation), solid=bool(brep.IsSolid),
        closed=all(e.Valence == Rhino.Geometry.EdgeAdjacency.Interior for e in brep.Edges),
        geometry=host["_interchange_brep_record"](brep, include_samples=False))


def run(operation, iterations, host):
    import Rhino
    import System
    validate(operation, iterations)
    path = operation.get("artifact_path")
    if not path:
        raise ValueError("document B-rep admission requires a shared artifact from compare mode")
    if path.startswith("/"): path = "Z:" + path.replace("/", "\\")
    model = Rhino.FileIO.File3dm.Read(path)
    if model is None: raise ValueError("cannot read admission artifact")
    replacement = attrs = None
    key, group = System.Guid.Empty, -1
    document = Rhino.RhinoDoc.ActiveDoc
    settings = Rhino.DocObjects.ObjectEnumeratorSettings()
    settings.NormalObjects = settings.HiddenObjects = settings.LockedObjects = True
    def objects(): return list(document.Objects.GetObjectList(settings))
    before = set(o.Id for o in objects())
    selected_before = [o.Id for o in objects() if o.IsSelected(False)]
    def geometry(brep): return geometry_record(brep, host)
    def snapshot():
        obj = document.Objects.FindId(key)
        if obj is None: raise ValueError("admission lost object identity")
        return dict(geometry=geometry(obj.Geometry), name=obj.Attributes.Name,
            group_count=int(obj.Attributes.GroupCount), selected=bool(obj.IsSelected(False)),
            current_layer=obj.Attributes.LayerIndex == document.Layers.CurrentLayerIndex,
            object_count=len([o for o in objects() if o.Id not in before]), identity_preserved=obj.Id == key)
    try:
        items = list(model.Objects)
        if len(items) != 1 or not isinstance(items[0].Geometry, Rhino.Geometry.Brep):
            raise ValueError("admission artifact must contain one B-rep")
        source = items[0].Geometry
        if not source.IsValid: raise ValueError("invalid admission B-rep")
        input_record = geometry(source)
        # Never derive this from the inserted object; both engines must see the
        # same replacement source even when their insertion behavior diverges.
        replacement = source.DuplicateBrep()
        replacement.Flip()
        replacement_input = geometry(replacement)
        document.Objects.UnselectAll()
        attrs = Rhino.DocObjects.ObjectAttributes()
        attrs.Name = "Source"
        attrs.LayerIndex = document.Layers.CurrentLayerIndex
        if operation.get("insertion", "generic") == "no_kink":
            key = document.Objects.AddBrep(source, attrs, None, False, False)
        else:
            key = document.Objects.Add(source, attrs)
        if key == System.Guid.Empty: raise ValueError("admission insertion failed")
        group = document.Groups.Add("Viboceros admission " + str(System.Guid.NewGuid()), [key])
        if group < 0: raise ValueError("admission group creation failed")
        if operation.get("selected", False): document.Objects.Select(key)
        inserted = snapshot()
        replaced = bool(document.Objects.Replace(key, replacement))
        return dict(input=input_record, inserted=inserted, replacement_input=replacement_input,
            replacement=snapshot(), replaced=replaced, source_unchanged=geometry(source)==input_record,
            replacement_unchanged=geometry(replacement)==replacement_input), 0
    finally:
        if key != System.Guid.Empty: document.Objects.Delete(key, True)
        if group >= 0: document.Groups.Delete(group)
        document.Objects.UnselectAll()
        for previous in selected_before: document.Objects.Select(previous)
        if attrs is not None: attrs.Dispose()
        if replacement is not None: replacement.Dispose()
        model.Dispose()
