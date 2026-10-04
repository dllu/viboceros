"""Closed Maelstrom point/Enter getter recipes on owned point geometry."""
import math
import re

if globals().get("__package__"):
    from .maelstrom_command_probe import scalar, vector, radius
else:
    from maelstrom_command_probe import scalar, vector, radius


def validate(op):
    fields = {"op", "id", "center", "normal", "radius0", "radius1", "angles", "copy", "rigid", "postselect"}
    if (not isinstance(op, dict) or set(op) != fields
        or op["op"] != "maelstrom_input_command"
        or not isinstance(op["id"], str) or re.match(r"^[A-Za-z0-9_-]{1,80}\Z", op["id"]) is None
        or not vector(op["center"]) or not vector(op["normal"]) or not any(op["normal"])
        or any(v is not None and not radius(v) for v in (op["radius0"], op["radius1"]))
        or not isinstance(op["angles"], list) or not 1 <= len(op["angles"]) <= 4
        or any(v is not None and not (scalar(v, 1440) or vector(v)) for v in op["angles"])
        or any(type(op[k]) is not bool for k in ("copy", "rigid", "postselect"))
        or (len(op["angles"]) > 1 and not op["copy"])
        or any(v is None for v in op["angles"][:-1])):
        raise ValueError("invalid bounded Maelstrom input recipe")


def request():
    cases = []
    def add(**changes):
        op = dict(op="maelstrom_input_command", id="maelstrom-input-" + str(len(cases)),
                  center=[0., 0., 0.], normal=[0., 0., 1.], radius0=2., radius1=5.,
                  angles=[90.], copy=False, rigid=False, postselect=False)
        op.update(changes)
        validate(op)
        cases.append(op)
    add(radius0=4.)
    add(radius0=None)
    add(radius1=None)
    add(angles=[None])
    for point in ([5.,0.,0.], [0.,5.,0.], [-5.,0.,0.], [0.,-5.,0.],
                  [3.,3.,7.], [0.,0.,0.], [0.,0.,5.]):
        add(angles=[point])
    add(angles=[[0.,-5.,0.], [0.,5.,0.], None], copy=True)
    add(angles=[[0.,5.,0.]], postselect=True)
    add(angles=[[0.,5.,0.]], rigid=True)
    add(center=[1.,2.,3.], normal=[1.,2.,3.], angles=[[4.,5.,7.]])
    add(radius0=[0.,0.,2.], angles=[[0.,5.,0.]])
    add(radius0=[0.,1.,math.sqrt(3.)], angles=[[3.,3.,7.]])
    add(radius1=0., angles=[[0.,5.,0.]])
    return dict(protocol_version=1, iterations=1, operations=cases)


def run(op, host):
    from join_probe import observe_command
    from twist_command_probe import source_geometry
    from number_token import number_token
    validate(op)
    Rhino, System = host["Rhino"], host["System"]
    doc = Rhino.RhinoDoc.ActiveDoc
    if list(doc.Objects):
        raise ValueError("Maelstrom input probe requires an empty owned document")
    vp = doc.Views.ActiveView.ActiveViewport
    saved_plane = vp.GetConstructionPlane()
    ids, owned = [], []
    def snapshot():
        return [dict(source=ids.index(obj.Id) if obj.Id in ids else None,
                     selected=bool(obj.IsSelected(False)),
                     point=host["_xyz"](obj.Geometry.Location))
                for obj in sorted(list(doc.Objects), key=lambda obj: obj.RuntimeSerialNumber)]
    def token(value):
        if value is None:
            return "_Enter"
        if isinstance(value, list):
            return "w" + ",".join(number_token(v) for v in value)
        return number_token(value)
    try:
        vp.SetConstructionPlane(Rhino.Geometry.Plane(host["_point"](op["center"]), Rhino.Geometry.Vector3d(*op["normal"])))
        serial = doc.BeginUndoRecord("Maelstrom input sources")
        try:
            owned = source_geometry("Points", host)
            ids = [doc.Objects.Add(g) for g in owned]
            if any(key == System.Guid.Empty for key in ids):
                raise ValueError("Maelstrom input source insertion failed")
            if not op["postselect"]:
                for key in ids:
                    doc.Objects.Select(key)
        finally:
            doc.EndUndoRecord(serial)
        before = snapshot()
        selection = " ".join("_SelID " + str(key) for key in ids) + " _Enter " if op["postselect"] else ""
        macro = "_Maelstrom " + selection + token(op["center"]) + " " + token(op["radius0"])
        macro += " _Copy=_" + ("Yes" if op["copy"] else "No") + " _Rigid=_" + ("Yes" if op["rigid"] else "No")
        macro += " " + token(op["radius1"])
        if op["radius1"] is not None:
            macro += " " + " ".join(token(v) for v in op["angles"])
        macro += " _Cancel"
        marker = "Viboceros Maelstrom input " + str(System.Guid.NewGuid())
        Rhino.RhinoApp.WriteLine(marker)
        host["_record_progress"](op["id"] + " " + macro)
        success, after, events = observe_command(Rhino.Commands.Command, "Maelstrom",
            lambda: Rhino.RhinoApp.RunScript(macro, True), snapshot, lambda: [], True)
        history = Rhino.RhinoApp.CommandHistoryWindowText.split(marker, 1)[1].strip()
        undo = redo = None
        # Even unchanged morphs record Undo; an Enter before placement does not.
        # Undo of source setup is visibly empty when the command made no record.
        Rhino.RhinoApp.RunScript("_Undo", False)
        undo = snapshot()
        Rhino.RhinoApp.RunScript("_Redo", False)
        redo = snapshot()
        return dict(before=before, after=after, undo=undo, redo=redo, success=success,
                    events=events, history=history, script_macro=macro), 0
    finally:
        for obj in list(doc.Objects):
            doc.Objects.Delete(obj.Id, True)
        for geom in owned:
            geom.Dispose()
        vp.SetConstructionPlane(saved_plane.Plane)
