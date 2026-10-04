"""Bounded public grip transforms in an empty, owned Rhino document."""
import re
import math

KINDS = ("curve", "rational", "closed", "periodic", "line", "polyline", "circle", "arc",
         "surface", "surface_closed", "mesh", "closing")
COUNTS = dict(curve=4, rational=4, closed=4, periodic=4, line=2, polyline=4,
              circle=8, arc=5, surface=9, surface_closed=12, mesh=5, closing=4)
COMMANDS = {
    "move": ("Move", "_Move 0,0,0 1,2,3"),
    "zero": ("Move", "_Move 0,0,0 0,0,0"),
    "rotate": ("Rotate", "_Rotate _Copy=_No 0,0,0 30"),
    "scale": ("Scale", "_Scale _Copy=_No 0,0,0 2"),
    "mirror": ("Mirror", "_Mirror _Copy=_No 0,0,0 0,1,0"),
    "copy": ("Copy", "_Copy 0,0,0 1,2,3 _Enter"),
}


def validate(op):
    if (not isinstance(op, dict) or set(op) != {"op", "id", "source", "selected", "parent", "point", "command", "off", "inputs"}
            or op["op"] != "grip_transform" or not isinstance(op["id"], str)
            or re.match(r"^[A-Za-z0-9_-]{1,80}\Z", op["id"]) is None
            or not isinstance(op["source"], str) or op["source"] not in KINDS
            or not isinstance(op["command"], str) or op["command"] not in COMMANDS
            or type(op["parent"]) is not bool or type(op["point"]) is not bool or type(op["off"]) is not bool
            or op["inputs"] not in ("auto", "all")
            or not isinstance(op["selected"], list)
            or any(type(i) is not int or not 0 <= i < COUNTS[op["source"]] for i in op["selected"])
            or len(set(op["selected"])) != len(op["selected"])):
        raise ValueError("invalid bounded grip transform recipe")
    if ((op["inputs"] == "auto") != bool(op["selected"])
            or (op["inputs"] == "all" and (op["parent"] or op["point"]))):
        raise ValueError("grip transform macro must match source selection")


def request():
    rows = [(kind, [0], False, False, "move") for kind in KINDS if kind != "closing"]
    rows += [(kind, [0, 2], False, False, command)
             for kind, command in (("rational", "rotate"), ("surface", "scale"),
                                   ("mesh", "mirror"), ("curve", "copy"),
                                   ("surface", "copy"), ("mesh", "copy"))]
    rows += [("curve", [0, 2], True, False, "move"),
             ("surface", [0, 2], True, False, "move"),
             ("mesh", [0, 2], True, False, "move"),
             ("curve", [0, 2], False, True, "move"),
             ("curve", [0], False, False, "zero"),
             ("mesh", [0], False, False, "zero")]
    operations = [dict(op="grip_transform", id="grip-transform-"+str(i), source=kind,
                       selected=picks, parent=parent, point=point, command=command, off=False, inputs="auto")
                  for i, (kind, picks, parent, point, command) in enumerate(rows)]
    for i, kind, command in ((23, "curve", "move"), (24, "curve", "copy"), (25, "mesh", "move")):
        operations.append(dict(op="grip_transform", id="grip-transform-"+str(i), source=kind,
                               selected=[0], parent=False, point=False, command=command, off=True, inputs="auto"))
    for i, kind in ((26, "curve"), (27, "mesh")):
        operations.append(dict(op="grip_transform", id="grip-transform-"+str(i), source=kind,
                               selected=[], parent=False, point=False, command="move", off=False, inputs="all"))
    for i, parent, point in ((28, True, False), (29, False, True)):
        operations.append(dict(op="grip_transform", id="grip-transform-"+str(i), source="curve",
                               selected=[0, 2], parent=parent, point=point, command="copy", off=False, inputs="auto"))
    for i, command in ((30, "move"), (31, "copy")):
        operations.append(dict(op="grip_transform", id="grip-transform-"+str(i), source="closing",
                               selected=[3], parent=False, point=False, command=command, off=False, inputs="auto"))
    for op in operations:
        validate(op)
    return dict(protocol_version=1, iterations=1, operations=operations)


def create_source(kind, host):
    Rhino = host["Rhino"]
    cardinal = [[2., 0., 0.], [0., 2., 0.], [-2., 0., 0.], [0., -2., 0.]]
    if kind == "closing":
        cardinal[-1] = [1., -2., -3.]
    if kind in ("curve", "rational", "closed", "periodic", "closing"):
        controls = cardinal + (cardinal[:1] if kind == "closed" else cardinal[:2] if kind == "periodic" else [])
        curve = Rhino.Geometry.NurbsCurve(3, True, 3, len(controls))
        for i, p in enumerate(controls):
            curve.Points.SetPoint(i, host["_point"](p), [1., 2., .5, 1.][i] if kind == "rational" else 1.)
        if kind == "periodic":
            curve.Knots.CreatePeriodicKnots(1.)
        else:
            curve.Knots.CreateUniformKnots(1.)
        return curve
    if kind == "line":
        return Rhino.Geometry.LineCurve(host["_point"](cardinal[0]), host["_point"](cardinal[1]))
    if kind == "polyline":
        return Rhino.Geometry.PolylineCurve([host["_point"](p) for p in cardinal+cardinal[:1]])
    if kind in ("circle", "arc"):
        circle = Rhino.Geometry.Circle(host["_point"]([0., 0., 0.]), 2.)
        return Rhino.Geometry.ArcCurve(circle if kind == "circle" else Rhino.Geometry.Arc(circle, math.pi))
    if kind in ("surface", "surface_closed"):
        count_u = 5 if kind == "surface_closed" else 3
        surface = Rhino.Geometry.NurbsSurface.Create(3, kind == "surface", 3, 3, count_u, 3)
        for u in range(count_u):
            for v in range(3):
                p = (cardinal[u % 4][:2]+[float(v)]) if kind == "surface_closed" else [float(u), float(v), .25*u*v]
                weight = 1. + .125*u*v if kind == "surface" else 1.
                surface.Points.SetPoint(u, v, host["_point"](p), weight)
        surface.KnotsU.CreateUniformKnots(1.)
        surface.KnotsV.CreateUniformKnots(1.)
        return surface
    mesh = Rhino.Geometry.Mesh()
    mesh.Vertices.UseDoublePrecisionVertices = True
    for p in cardinal+cardinal[:1]:
        mesh.Vertices.Add(host["_point"](p))
    mesh.Faces.AddFace(0, 1, 2, 3)
    mesh.Faces.AddFace(4, 2, 3)
    return mesh


def run(op, host):
    validate(op)
    Rhino, System = host["Rhino"], host["System"]
    doc = Rhino.RhinoDoc.ActiveDoc
    if list(doc.Objects):
        raise ValueError("grip transform requires an empty owned document")
    from join_probe import observe_command
    source_id, point_id = None, None

    def snapshot():
        rows = []
        for obj in sorted(list(doc.Objects), key=lambda obj: obj.RuntimeSerialNumber):
            geometry = obj.Geometry
            row = dict(role="source" if obj.Id == source_id else "point" if obj.Id == point_id else "output",
                       selected=bool(obj.IsSelected(False)), grips_on=bool(obj.GripsOn),
                       grips=[dict(index=int(g.Index), point=host["_xyz"](g.CurrentLocation), selected=bool(g.IsSelected(False)))
                              for g in (obj.GetGrips() or [])], kind=geometry.GetType().Name,
                       name=obj.Attributes.Name)
            if isinstance(geometry, Rhino.Geometry.Curve):
                row["curve"] = host["_nurbs_curve_definition"](geometry.ToNurbsCurve())
            elif isinstance(geometry, Rhino.Geometry.Brep):
                row["surface"] = host["_nurbs_surface_definition"](geometry.Surfaces[0].ToNurbsSurface())
            elif isinstance(geometry, Rhino.Geometry.Mesh):
                row["mesh"] = host["_polygon_mesh_value"](geometry)
            elif isinstance(geometry, Rhino.Geometry.Point):
                row["point"] = host["_xyz"](geometry.Location)
            else:
                raise ValueError("unexpected grip transform output")
            rows.append(row)
        return rows

    try:
        serial = doc.BeginUndoRecord("Grip transform sources")
        try:
            geometry = create_source(op["source"], host)
            attributes = Rhino.DocObjects.ObjectAttributes()
            attributes.Name = "grip source"
            source_id = doc.Objects.Add(geometry, attributes)
            point_id = doc.Objects.AddPoint(host["_point"]([8., 0., 0.])) if op["point"] else None
            if source_id == System.Guid.Empty or point_id == System.Guid.Empty:
                raise ValueError("grip source insertion failed")
        finally:
            doc.EndUndoRecord(serial)
        source = doc.Objects.FindId(source_id)
        source.GripsOn = True
        grips = source.GetGrips()
        if not grips or any(i >= len(grips) for i in op["selected"]):
            raise ValueError("grip source unavailable")
        for index in op["selected"]:
            grips[index].Select(True)
        if op["parent"]:
            doc.Objects.Select(source_id)
        if op["point"]:
            doc.Objects.Select(point_id)
        before = snapshot()
        name, macro = COMMANDS[op["command"]]
        if op["inputs"] == "all":
            macro = macro.replace("_"+name+" ", "_"+name+" _SelAll _Enter ", 1)
        host["_record_progress"](op["id"]+" "+macro)
        success, after, events = observe_command(Rhino.Commands.Command, name,
            lambda: Rhino.RhinoApp.RunScript(macro, True), snapshot, lambda: [], True)
        after_script = snapshot()
        points_off = None
        if op["off"]:
            Rhino.RhinoApp.RunScript("_PointsOff", True)
            points_off = snapshot()
        Rhino.RhinoApp.RunScript("_Undo", False)
        undo = snapshot()
        Rhino.RhinoApp.RunScript("_Redo", False)
        redo = snapshot()
        result = dict(before=before, after=after, after_script=after_script, undo=undo, redo=redo,
                      success=success, events=events, script_macro=macro)
        if points_off is not None:
            result["points_off"] = points_off
        return result, 0
    finally:
        for obj in list(doc.Objects):
            if obj.GripsOn:
                obj.GripsOn = False
            doc.Objects.Delete(obj.Id, True)
