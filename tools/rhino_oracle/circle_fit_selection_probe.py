"""Owned Circle FitPoints selection, minimum-count and cancellation captures."""
import math
import re


def validate(op):
    if (not isinstance(op, dict) or set(op) != {"op", "id", "points", "preselected", "line", "inputs"}
            or op["op"] != "circle_fit_selection"
            or not isinstance(op["id"], str) or re.match(r"^[A-Za-z0-9_-]{1,80}\Z", op["id"]) is None
            or not isinstance(op["points"], list) or not 0 <= len(op["points"]) <= 64
            or any(not isinstance(p, list) or len(p) != 3
                   or any(type(x) not in (int, float) or not -100 <= x <= 100 for x in p) for p in op["points"])
            or not isinstance(op["preselected"], list)
            or any(type(i) is not int or not 0 <= i < len(op["points"]) for i in op["preselected"])
            or len(set(op["preselected"])) != len(op["preselected"])
            or type(op["line"]) is not bool or op["inputs"] not in ("auto", "all", "enter")):
        raise ValueError("invalid bounded Circle FitPoints selection capture")
    if ((op["inputs"] == "auto") != (len(op["preselected"]) >= 3)):
        raise ValueError("Circle selection macro must match preselection count")


def request():
    circle = [[2., 0., 0.], [0., 2., 0.], [-2., 0., 0.], [0., -2., 0.]]
    cases = [(circle[:3], [0, 1, 2], False, "auto"),
             (circle, [0, 2], False, "all"),
             (circle[:1], [0], False, "enter"),
             (circle, [], False, "enter"),
             (circle[:3], [0, 1, 2], True, "auto"),
             (circle[:2], [0, 1], True, "all"),
             (circle, [], True, "all"),
             ([[0., 0., 0.], [1., 0., 0.], [2., 0., 0.]], [0, 1, 2], False, "auto")]
    rows = [dict(op="circle_fit_selection", id="circle-fit-selection-"+str(i), points=p,
                 preselected=s, line=line, inputs=inputs) for i, (p, s, line, inputs) in enumerate(cases)]
    for op in rows:
        validate(op)
    return dict(protocol_version=1, iterations=1, operations=rows)


def run(op, host):
    from join_probe import observe_command
    validate(op)
    Rhino, System = host["Rhino"], host["System"]
    doc = Rhino.RhinoDoc.ActiveDoc
    if list(doc.Objects):
        raise ValueError("selection capture requires an empty owned document")
    ids = []
    line_id = None

    def circle_record(c):
        return dict(origin=host["_xyz"](c.Center), x=host["_xyz"](c.Plane.XAxis),
                    y=host["_xyz"](c.Plane.YAxis), normal=host["_xyz"](c.Normal), radius=c.Radius)

    def snapshot():
        rows = []
        for obj in sorted(list(doc.Objects), key=lambda obj: obj.RuntimeSerialNumber):
            row = dict(source=ids.index(obj.Id) if obj.Id in ids else None, selected=bool(obj.IsSelected(False)))
            if isinstance(obj.Geometry, Rhino.Geometry.Point):
                row.update(kind="point", point=host["_xyz"](obj.Geometry.Location))
            elif obj.Id == line_id:
                row.update(kind="line", point=host["_xyz"](obj.Geometry.PointAtStart), end=host["_xyz"](obj.Geometry.PointAtEnd))
            else:
                ok, circle = obj.Geometry.TryGetCircle()
                if not ok:
                    raise ValueError("unexpected Circle output")
                row.update(kind="circle", circle=circle_record(circle), seam=host["_xyz"](obj.Geometry.PointAtStart))
            rows.append(row)
        return rows

    try:
        serial = doc.BeginUndoRecord("Circle FitPoints selection sources")
        try:
            ids = [doc.Objects.AddPoint(host["_point"](p)) for p in op["points"]]
            if any(i == System.Guid.Empty for i in ids):
                raise ValueError("point insertion failed")
            if op["line"]:
                line_id = doc.Objects.AddLine(host["_point"]([10., 0., 0.]), host["_point"]([11., 1., 0.]))
                if line_id == System.Guid.Empty:
                    raise ValueError("line insertion failed")
                doc.Objects.Select(line_id)
            for i in op["preselected"]:
                doc.Objects.Select(ids[i])
        finally:
            doc.EndUndoRecord(serial)
        before = snapshot()
        marker = "Viboceros Circle FitPoints selection "+str(System.Guid.NewGuid())
        Rhino.RhinoApp.WriteLine(marker)
        tokens = {"auto": "", "all": "_SelAll _Enter", "enter": "_Enter"}[op["inputs"]]
        macro = "_Circle _FitPoints "+tokens
        if op["inputs"] == "enter" or (op["inputs"] == "all" and len(op["points"]) < 3):
            macro += " _Cancel _Cancel"
        host["_record_progress"](op["id"]+" "+macro)
        success, after, events = observe_command(Rhino.Commands.Command, "Circle",
            lambda: Rhino.RhinoApp.RunScript(macro, True), snapshot, lambda: [], True)
        after_script = snapshot()
        history = Rhino.RhinoApp.CommandHistoryWindowText.split(marker, 1)[1].strip()
        Rhino.RhinoApp.RunScript("_Undo", False)
        undo = snapshot()
        Rhino.RhinoApp.RunScript("_Redo", False)
        redo = snapshot()
        return dict(before=before, after=after, after_script=after_script, undo=undo, redo=redo,
                    success=success, events=events, history=history, script_macro=macro), 0
    finally:
        for obj in list(doc.Objects):
            doc.Objects.Delete(obj.Id, True)
