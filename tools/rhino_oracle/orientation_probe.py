"""Public document insertion/replacement and Flip audit in an owned session.

This diagnostic retains Rhino-created definitions at every state. It is not an
identical-source native replay, and no orientation is inferred from volume sign.
"""
import re


def validate(operation):
    if (not isinstance(operation, dict) or set(operation) - {"preselect", "insertion", "replace_flip", "flip"}
            != {"op", "id", "sources"} or operation.get("op") != "orientation_audit"
            or not isinstance(operation["id"], str)
            or re.match(r"^[A-Za-z0-9_.-]{1,100}\Z", operation["id"]) is None):
        raise ValueError("invalid orientation audit fields")
    for key in ("preselect", "replace_flip", "flip"):
        if type(operation.get(key, False)) is not bool:
            raise ValueError("orientation options must be boolean")
    if operation.get("insertion", "generic") not in ("generic", "no_kink"):
        raise ValueError("invalid orientation insertion overload")
    sources = operation["sources"]
    if not isinstance(sources, list) or not 1 <= len(sources) <= 8:
        raise ValueError("orientation audit requires 1 to 8 sources")
    def source(spec, depth=0):
        if (not isinstance(spec, dict) or set(spec) - {"reversed", "offset", "size", "parts"} != {"kind"}
                or type(spec.get("reversed", False)) is not bool or depth > 2):
            raise ValueError("invalid orientation source")
        if spec["kind"] not in ("box", "open_box", "plane", "sphere", "mesh", "line", "point", "compound"):
            raise ValueError("unknown orientation source")
        if type(spec.get("size", 1)) is not int or spec.get("size", 1) not in (1, 2, 4):
            raise ValueError("unsupported orientation size")
        if type(spec.get("offset", 0)) is not int or abs(spec.get("offset", 0)) > 100:
            raise ValueError("invalid orientation offset")
        if spec["kind"] == "compound":
            if "size" in spec or "offset" in spec:
                raise ValueError("compound placement belongs to its parts")
            parts = spec.get("parts")
            if not isinstance(parts, list) or not 1 <= len(parts) <= 4:
                raise ValueError("invalid orientation compound")
            for part in parts:
                source(part, depth + 1)
                if part.get("kind") not in ("box", "open_box", "plane", "sphere"):
                    raise ValueError("compound parts must be B-reps")
        elif "parts" in spec:
            raise ValueError("unexpected compound parts")
        if spec["kind"] == "point" and (spec.get("reversed", False) or operation.get("replace_flip", False)):
            raise ValueError("points have no direction to reverse")
    for spec in sources: source(spec)


def run(operation, tolerance, host):
    import Rhino
    import System
    from join_probe import observe_command
    validate(operation)
    owned = []
    def reverse(g):
        if isinstance(g, Rhino.Geometry.Brep): g.Flip()
        elif isinstance(g, Rhino.Geometry.Mesh): g.Flip(True, True, True)
        elif isinstance(g, Rhino.Geometry.Curve): g.Reverse()
        else: raise ValueError("cannot reverse this source")
    def source(spec):
        x, s = float(spec.get("offset", 0)), float(spec.get("size", 1))
        lo, hi = Rhino.Geometry.Point3d(x-s, -s, -s), Rhino.Geometry.Point3d(x+s, s, s)
        kind = spec["kind"]
        if kind in ("box", "open_box"):
            g = Rhino.Geometry.Brep.CreateFromBox(Rhino.Geometry.BoundingBox(lo, hi))
            if kind == "open_box": g.Faces.RemoveAt(g.Faces.Count-1)
        elif kind == "sphere": g = Rhino.Geometry.Sphere(Rhino.Geometry.Point3d(x, 0, 0), s).ToBrep()
        elif kind == "plane":
            plane = Rhino.Geometry.Plane(Rhino.Geometry.Point3d(x, 0, 0), Rhino.Geometry.Vector3d.ZAxis)
            surface = Rhino.Geometry.PlaneSurface(plane, Rhino.Geometry.Interval(-s, s), Rhino.Geometry.Interval(-s, s))
            try: g = surface.ToBrep()
            finally: surface.Dispose()
        elif kind == "mesh":
            g = Rhino.Geometry.Mesh()
            g.Vertices.UseDoublePrecisionVertices = True
            for p in ((x,0,0), (x+3*s,0,0), (x,4*s,0), (x,0,5*s)): g.Vertices.Add(*p)
            for f in ((0,2,1), (0,1,3), (0,3,2), (1,2,3)): g.Faces.AddFace(*f)
        elif kind == "line":
            g = Rhino.Geometry.LineCurve(Rhino.Geometry.Point3d(x,0,0), Rhino.Geometry.Point3d(x+s,s,s))
        elif kind == "point": g = Rhino.Geometry.Point(Rhino.Geometry.Point3d(x,0,0))
        else:
            g = Rhino.Geometry.Brep()
            for part in spec["parts"]: g.Append(source(part))
        owned.append(g)
        if spec.get("reversed", False): reverse(g)
        if not g.IsValid: raise ValueError("invalid constructed orientation source")
        return g
    def geometry(g):
        if isinstance(g, Rhino.Geometry.Brep):
            record = host["_interchange_brep_record"](g)
            for edge in record["edges"]: edge["curve"].pop("samples")
            for face in record["faces"]:
                face.pop("samples")
                for loop in face["loops"]:
                    for trim in loop: trim.pop("lifted")
            record.update(type="brep", solid=bool(g.IsSolid), orientation=str(g.SolidOrientation))
            mass = Rhino.Geometry.VolumeMassProperties.Compute(g) if g.IsSolid else None
            try: record["volume"] = None if mass is None else float(mass.Volume)
            finally:
                if mass is not None: mass.Dispose()
            return record
        if isinstance(g, Rhino.Geometry.Mesh):
            record = host["_polygon_mesh_value"](g)
            record.update(type="mesh", solid=bool(g.IsClosed), orientation=int(g.SolidOrientation()))
            return record
        if isinstance(g, Rhino.Geometry.Curve):
            return dict(type="curve", definition=host["_nurbs_curve_definition"](g))
        return dict(type="point", point=host["_xyz"](g.Location))
    document = Rhino.RhinoDoc.ActiveDoc
    settings = Rhino.DocObjects.ObjectEnumeratorSettings()
    settings.NormalObjects = settings.HiddenObjects = settings.LockedObjects = True
    def objects(): return list(document.Objects.GetObjectList(settings))
    before = set(obj.Id for obj in objects())
    selected_before = [obj.Id for obj in objects() if obj.IsSelected(False)]
    ids, groups = [], []
    def snapshot():
        result = []
        for index, key in enumerate(ids):
            obj = document.Objects.FindId(key)
            if obj is None: raise ValueError("orientation operation lost source identity")
            result.append(dict(geometry=geometry(obj.Geometry), selected=bool(obj.IsSelected(False)),
                name=obj.Attributes.Name, groups=int(obj.Attributes.GroupCount),
                current_layer=obj.Attributes.LayerIndex == document.Layers.CurrentLayerIndex))
        if len([obj for obj in objects() if obj.Id not in before]) != len(ids):
            raise ValueError("orientation operation changed object count")
        return result
    try:
        document.Objects.UnselectAll()
        constructed, inserted = [], []
        for index, spec in enumerate(operation["sources"]):
            g = source(spec)
            constructed.append(geometry(g))
            attrs = Rhino.DocObjects.ObjectAttributes()
            try:
                attrs.Name = "source-%d" % index
                if operation.get("insertion", "generic") == "no_kink" and isinstance(g, Rhino.Geometry.Brep):
                    key = document.Objects.AddBrep(g, attrs, None, False, False)
                else: key = document.Objects.Add(g, attrs)
            finally: attrs.Dispose()
            if key == System.Guid.Empty: raise ValueError("orientation source insertion failed")
            ids.append(key)
            inserted.append(geometry(document.Objects.FindId(key).Geometry))
            if geometry(g) != constructed[-1]: raise ValueError("insertion mutated caller geometry")
        group = document.Groups.Add("Viboceros orientation " + str(System.Guid.NewGuid()), ids)
        if group < 0: raise ValueError("orientation group creation failed")
        groups.append(group)
        replacement = None
        if operation.get("replace_flip", False):
            for key in ids:
                g = document.Objects.FindId(key).Geometry.Duplicate()
                owned.append(g)
                reverse(g)
                if not document.Objects.Replace(key, g): raise ValueError("orientation replacement failed")
            replacement = snapshot()
        command = None
        if operation.get("flip", False):
            if operation.get("preselect", False):
                for key in ids: document.Objects.Select(key)
                macro = "_Flip _Enter"
            else: macro = "_Flip " + " ".join("_SelID %s" % key for key in ids) + " _Enter"
            marker = "Viboceros orientation " + str(System.Guid.NewGuid())
            Rhino.RhinoApp.WriteLine(marker)
            succeeded, outputs, events = observe_command(Rhino.Commands.Command, "Flip",
                lambda: Rhino.RhinoApp.RunScript(macro, True), snapshot, lambda: [], True)
            history = Rhino.RhinoApp.CommandHistoryWindowText.split(marker, 1)
            if len(history) != 2: raise ValueError("orientation history marker missing")
            command = dict(succeeded=succeeded, objects=outputs, events=events, history=history[1].strip())
        return dict(constructed=constructed, inserted=inserted, replacement=replacement, command=command), 0
    finally:
        Rhino.RhinoApp.RunScript("!", False)
        for obj in objects():
            if obj.Id not in before: document.Objects.Delete(obj.Id, True)
        for group in groups: document.Groups.Delete(group)
        document.Objects.UnselectAll()
        for key in selected_before: document.Objects.Select(key)
        for g in reversed(owned): g.Dispose()
