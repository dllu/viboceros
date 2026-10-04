"""Owned public Circle FitPoints grip input and lifecycle observations."""
import re


def validate(op):
    fields = {"op", "id", "source", "controls", "weights", "selected", "points", "selected_points", "inputs", "off"}
    if (not isinstance(op, dict) or set(op) != fields or op["op"] not in ("circle_fit_grips", "circle_fit_grips_commands")
            or not isinstance(op["id"], str) or re.match(r"^[A-Za-z0-9_-]{1,80}\Z", op["id"]) is None
            or op["source"] not in ("curve", "surface", "mesh") or op["inputs"] not in ("auto", "all", "enter")
            or type(op["off"]) is not bool):
        raise ValueError("invalid Circle grip capture")
    for name in ("controls", "points"):
        if (not isinstance(op[name], list) or len(op[name]) > 64
                or any(not isinstance(p, list) or len(p) != 3
                       or any(type(x) not in (int, float) or not -100 <= x <= 100 for x in p) for p in op[name])):
            raise ValueError("invalid bounded grip coordinates")
    n = len(op["controls"])
    if ((op["source"] == "curve" and n < 3) or (op["source"] == "surface" and n != 4)
            or (op["source"] == "mesh" and n not in (4, 5))):
        raise ValueError("invalid grip source size")
    weights = op["weights"]
    if (not isinstance(weights, list) or (weights and (op["source"] != "curve" or len(weights) != n))
            or any(type(w) not in (int, float) or not .125 <= w <= 8 for w in weights)):
        raise ValueError("invalid rational grip weights")
    for indices, points in (("selected", "controls"), ("selected_points", "points")):
        if (not isinstance(op[indices], list)
                or any(type(i) is not int or not 0 <= i < len(op[points]) for i in op[indices])
                or len(set(op[indices])) != len(op[indices])):
            raise ValueError("invalid selected grip indices")
    count = (n if op["op"] == "circle_fit_grips_commands" else len(op["selected"])) + len(op["selected_points"])
    if (op["inputs"] == "auto") != (count >= 3):
        raise ValueError("Circle grip macro must match preselection count")


def request():
    cardinal = [[2., 0., 0.], [0., 2., 0.], [-2., 0., 0.], [0., -2., 0.]]
    grid = [[-2., -2., 0.], [-2., 2., 0.], [2., -2., 0.], [2., 2., 0.]]
    cases = [
        ("curve", cardinal, [], [0, 1, 2], [], [], "auto"),
        ("curve", cardinal, [], [], [], [], "all"),
        ("curve", cardinal, [], [0, 2], [], [], "all"),
        ("curve", cardinal, [], [0, 2], [], [], "enter"),
        ("curve", cardinal, [1., 2., .5, 1.], [0, 1, 2, 3], [], [], "auto"),
        ("surface", grid, [], [0, 1, 2, 3], [], [], "auto"),
        ("surface", grid, [], [], [], [], "all"),
        ("mesh", cardinal, [], [0, 1, 2, 3], [], [], "auto"),
        ("mesh", cardinal, [], [], [], [], "all"),
        ("curve", cardinal, [], [0], cardinal[1:3], [0, 1], "auto"),
        ("curve", [[0., 0., 0.], [1., 0., 0.], [2., 0., 0.]], [], [0, 1, 2], [], [], "auto"),
        ("mesh", cardinal + cardinal[:1], [], [], [], [], "all"),
    ]
    ops = [dict(op="circle_fit_grips", id="circle-fit-grips-"+str(i), source=source,
                controls=controls, weights=weights, selected=selected, points=points,
                selected_points=selected_points, inputs=inputs, off=False)
           for i, (source, controls, weights, selected, points, selected_points, inputs) in enumerate(cases)]
    for index in (1, 6, 8):
        op = dict(ops[index], op="circle_fit_grips_commands", id="circle-fit-grip-commands-"+ops[index]["source"],
                  selected=list(range(len(ops[index]["controls"]))), inputs="auto", off=True)
        ops.append(op)
    ops.append(dict(ops[9], id="circle-fit-grips-mixed-points-off", off=True))
    for op in ops:
        validate(op)
    return dict(protocol_version=1, iterations=1, operations=ops)


def run(op, host):
    from join_probe import observe_command
    validate(op)
    Rhino, System = host["Rhino"], host["System"]
    doc = Rhino.RhinoDoc.ActiveDoc
    if list(doc.Objects):
        raise ValueError("grip capture requires an empty owned document")
    source_id = None
    point_ids = []

    def snapshot():
        rows = []
        for obj in sorted(list(doc.Objects), key=lambda obj: obj.RuntimeSerialNumber):
            row = dict(selected=bool(obj.IsSelected(False)))
            if obj.Id == source_id:
                row.update(kind=op["source"], grips_on=bool(obj.GripsOn), grips=[
                    dict(index=int(g.Index), point=host["_xyz"](g.CurrentLocation), selected=bool(g.IsSelected(False)))
                    for g in (obj.GetGrips() or [])])
                if op["source"] == "curve":
                    row["definition"] = host["_nurbs_curve_definition"](obj.Geometry)
                elif op["source"] == "surface":
                    surface = obj.Geometry.Surfaces[0] if isinstance(obj.Geometry, Rhino.Geometry.Brep) else obj.Geometry
                    row["definition"] = host["_nurbs_surface_definition"](surface.ToNurbsSurface())
                else:
                    row["vertices"] = [host["_xyz"](p) for p in obj.Geometry.Vertices]
            elif obj.Id in point_ids:
                row.update(kind="point", source=point_ids.index(obj.Id), point=host["_xyz"](obj.Geometry.Location))
            else:
                ok, c = obj.Geometry.TryGetCircle()
                if not ok:
                    raise ValueError("unexpected grip fit output")
                row.update(kind="circle", circle=dict(origin=host["_xyz"](c.Center), radius=c.Radius,
                    x=host["_xyz"](c.Plane.XAxis), y=host["_xyz"](c.Plane.YAxis), normal=host["_xyz"](c.Normal)),
                    seam=host["_xyz"](obj.Geometry.PointAtStart))
            rows.append(row)
        return rows

    try:
        serial = doc.BeginUndoRecord("Circle grip sources")
        try:
            controls = op["controls"]
            if op["source"] == "curve":
                geometry = Rhino.Geometry.NurbsCurve(3, True, 3, len(controls))
                for i, p in enumerate(controls):
                    geometry.Points.SetPoint(i, host["_point"](p), op["weights"][i] if op["weights"] else 1.)
                geometry.Knots.CreateUniformKnots(1.)
                source_id = doc.Objects.AddCurve(geometry)
            elif op["source"] == "surface":
                geometry = Rhino.Geometry.NurbsSurface.Create(3, False, 2, 2, 2, 2)
                for i, p in enumerate(controls):
                    geometry.Points.SetPoint(i // 2, i % 2, host["_point"](p))
                geometry.KnotsU.CreateUniformKnots(1.)
                geometry.KnotsV.CreateUniformKnots(1.)
                source_id = doc.Objects.AddSurface(geometry)
            else:
                geometry = Rhino.Geometry.Mesh()
                for p in controls:
                    geometry.Vertices.Add(host["_point"](p))
                geometry.Faces.AddFace(0, 1, 2, 3)
                if len(controls) == 5:
                    geometry.Faces.AddFace(4, 2, 3)
                source_id = doc.Objects.AddMesh(geometry)
            if source_id == System.Guid.Empty:
                raise ValueError("grip source insertion failed")
            point_ids = [doc.Objects.AddPoint(host["_point"](p)) for p in op["points"]]
            if any(i == System.Guid.Empty for i in point_ids):
                raise ValueError("grip companion point insertion failed")
        finally:
            doc.EndUndoRecord(serial)
        source = doc.Objects.FindId(source_id)
        if op["op"] == "circle_fit_grips_commands":
            doc.Objects.Select(source_id)
            if not Rhino.RhinoApp.RunScript("_PointsOn", True):
                raise ValueError("PointsOn failed")
            if not Rhino.RhinoApp.RunScript("_SelAll", True):
                raise ValueError("SelAll grips failed")
        else:
            source.GripsOn = True
        grips = source.GetGrips()
        if not grips or any(i >= len(grips) for i in op["selected"]):
            raise ValueError("source grips unavailable")
        for i in op["selected"]:
            grips[i].Select(True)
        for i in op["selected_points"]:
            doc.Objects.Select(point_ids[i])
        before = snapshot()
        marker = "Viboceros Circle grip input "+str(System.Guid.NewGuid())
        Rhino.RhinoApp.WriteLine(marker)
        # Complete fits must not receive idle Escape tokens: those turn off
        # selected grip displays. Only the insufficient-input recipe needs
        # cancellation to return from the unresolved Circle getter.
        macro = "_Circle _FitPoints " + {
            "auto": "", "all": "_SelAll _Enter", "enter": "_Enter _Cancel _Cancel"
        }[op["inputs"]]
        host["_record_progress"](op["id"]+" "+macro)
        success, after, events = observe_command(Rhino.Commands.Command, "Circle",
            lambda: Rhino.RhinoApp.RunScript(macro, True), snapshot, lambda: [], True)
        # Retain both the EndCommand and final script states so history
        # behavior is not inferred across any macro cancellation.
        after_script = snapshot()
        history = Rhino.RhinoApp.CommandHistoryWindowText.split(marker, 1)[1].strip()
        points_off = None
        if op["off"]:
            if not Rhino.RhinoApp.RunScript("_PointsOff", True):
                raise ValueError("PointsOff failed")
            points_off = snapshot()
        Rhino.RhinoApp.RunScript("_Undo", False)
        undo = snapshot()
        Rhino.RhinoApp.RunScript("_Redo", False)
        redo = snapshot()
        result = dict(before=before, after=after, after_script=after_script, undo=undo, redo=redo,
                      success=success, events=events, history=history, script_macro=macro)
        if points_off is not None:
            result["points_off"] = points_off
        return result, 0
    finally:
        for obj in list(doc.Objects):
            if obj.GripsOn:
                obj.GripsOn = False
            doc.Objects.Delete(obj.Id, True)
