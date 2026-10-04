"""Owned, bounded Maelstrom option edits, cancellation and remembered defaults."""
import re
import math

OPTIONS = ("Copy", "Rigid")


def validate(op):
    if (not isinstance(op, dict) or set(op) != {"op", "id", "steps"}
        or op["op"] != "maelstrom_options_command" or not isinstance(op["id"], str)
        or re.match(r"^[A-Za-z0-9_-]{1,80}\Z", op["id"]) is None
        or not isinstance(op["steps"], list) or not 1 <= len(op["steps"]) <= 64):
        raise ValueError("invalid bounded Maelstrom preferences workflow")
    for step in op["steps"]:
        if not isinstance(step, dict):
            raise ValueError("invalid Maelstrom preference step")
        if step.get("kind") in ("Undo", "Redo", "New") and set(step)=={"kind"}:
            continue
        if (step.get("kind")=="RememberCopyOptions" and set(step)=={"kind","enabled"}
            and type(step["enabled"]) is bool):
            continue
        if (step.get("kind") != "Maelstrom" or set(step)-{"pending_options", "radius0"} != {"kind","shape","options","finish"}
            or step["shape"] not in ("Points","Curve","Box")
            or step["finish"] not in ("Complete","CancelStart","CancelEnd","CopyThenCancel","CopyThenEnter")):
            raise ValueError("invalid Maelstrom command step")
        v=step.get("radius0",2.)
        if type(v) not in (int,float) or not 2.**-32 < v <= 100 or math.isnan(v) or math.isinf(v) or v != int(v):
            raise ValueError("invalid Maelstrom first radius")
        for key in ("options", "pending_options"):
            if key not in step:
                continue
            if (not isinstance(step[key],dict) or set(step[key])-set(OPTIONS)
                or any(type(v) is not bool for v in step[key].values())
               ):
                raise ValueError("invalid Maelstrom option values")
        if step["finish"].startswith("CopyThen"):
            if step["options"].get("Copy") is not True or "pending_options" not in step:
                raise ValueError("Maelstrom Copy placement requires pending options")
        elif "pending_options" in step:
            raise ValueError("pending Maelstrom options require a Copy placement")


def request():
    steps=[]
    def command(options=None,finish="CancelStart",pending=None):
        step=dict(kind="Maelstrom",shape="Curve",options=options or {},finish=finish)
        if pending is not None: step["pending_options"]=pending
        steps.append(step)
    command()
    command(dict(Rigid=True));command()
    command(dict(Rigid=True),"CancelEnd");command()
    command(dict(Rigid=True),"Complete")
    steps.extend([dict(kind="Undo"),dict(kind="Redo")])
    command(dict(Rigid=False),"CancelEnd");command()
    steps.append(dict(kind="RememberCopyOptions",enabled=False))
    command(dict(Copy=True),"Complete")
    steps.append(dict(kind="RememberCopyOptions",enabled=True))
    command(dict(Copy=True),"Complete")
    command(dict(Copy=False))
    command(dict(Copy=True,Rigid=False),"CopyThenCancel",dict(Rigid=True))
    steps.extend([dict(kind="Undo"),dict(kind="Redo")])
    command()
    command(dict(Copy=True,Rigid=False),"CopyThenEnter",dict(Rigid=True))
    steps.append(dict(kind="New"));command()
    command();steps[-1]["radius0"]=3.
    command(finish="CancelEnd");steps[-1]["radius0"]=4.
    command(finish="Complete");steps[-1]["radius0"]=5.
    steps.extend([dict(kind="Undo"),dict(kind="Redo"),dict(kind="New")])
    command()
    return dict(protocol_version=1,iterations=1,operations=[dict(op="maelstrom_options_command",id="maelstrom-preferences",steps=steps)])


def run(op, host):
    from join_probe import observe_command
    from twist_command_probe import source_geometry, geometry_record

    validate(op)
    Rhino, System = host["Rhino"], host["System"]
    doc = Rhino.RhinoDoc.ActiveDoc
    if list(doc.Objects):
        raise ValueError("Maelstrom preferences require an empty owned document")
    saved_tolerance = doc.ModelAbsoluteTolerance
    ids, records = {}, []

    def setup():
        doc.ModelAbsoluteTolerance = 1e-5
        serial = doc.BeginUndoRecord("Maelstrom preferences source setup")
        try:
            for shape in ("Points", "Curve", "Box"):
                geometry = source_geometry(shape, host)
                try:
                    ids[shape] = [doc.Objects.Add(g) for g in geometry]
                finally:
                    for g in geometry:
                        g.Dispose()
                if any(key == System.Guid.Empty for key in ids[shape]):
                    raise ValueError("Maelstrom preferences source insertion failed")
        finally:
            doc.EndUndoRecord(serial)

    def snapshot():
        return [
            dict(
                selected=bool(obj.IsSelected(False)),
                geometry=geometry_record(obj.Geometry, host),
            )
            for obj in sorted(
                list(Rhino.RhinoDoc.ActiveDoc.Objects),
                key=lambda obj: obj.RuntimeSerialNumber,
            )
        ]

    def select(shape):
        doc.Objects.UnselectAll()
        for key in ids[shape]:
            if not doc.Objects.Select(key):
                raise ValueError("Maelstrom preferences source selection failed")

    def invoke(macro, name):
        marker = "Viboceros Maelstrom preferences " + str(System.Guid.NewGuid())
        Rhino.RhinoApp.WriteLine(marker)
        host["_record_progress"](macro)
        success, after, events = observe_command(
            Rhino.Commands.Command,
            name,
            lambda: Rhino.RhinoApp.RunScript(macro, True),
            snapshot,
            lambda: [],
            True,
        )
        history = Rhino.RhinoApp.CommandHistoryWindowText.split(marker, 1)[1].strip()
        return dict(success=success, after=after, events=events, history=history)

    def query():
        select("Curve")
        before = snapshot()
        circle = invoke("_Maelstrom w0,0,0 _Cancel", "Maelstrom")
        radii = re.findall(r"Radius <([^>]+)>",circle["history"])
        if not radii or circle["after"] != before:
            raise ValueError("Maelstrom radius query changed sources or lacked a default")
        # The radius transcript is rounded to three decimals. Closed preference
        # recipes use integer radii so a query can reaccept the exact same value.
        first_radius = float(radii[0])
        if first_radius != int(first_radius):
            raise ValueError("Maelstrom preference radius query needs an exact integer default")
        result = invoke("_Maelstrom w0,0,0 " + str(int(first_radius)) + " _Cancel", "Maelstrom")
        if result["after"] != before:
            raise ValueError("unedited Maelstrom preference query changed its sources")
        matches = {
            name: re.findall(name + r"=(Yes|No)", result["history"]) for name in OPTIONS
        }
        if any(not values for values in matches.values()):
            raise ValueError(
                "Maelstrom preference defaults missing from transcript: " + result["history"]
            )
        result["defaults"] = {
            name: values[0] == "Yes" for name, values in matches.items()
        }
        result["first_radius"] = float(radii[0])
        result["first_radius_query"] = circle
        return result

    try:
        setup()
        for step in op["steps"]:
            before_query = query()
            kind = step["kind"]
            if kind == "Maelstrom":
                select(step["shape"])
                before = snapshot()
                options = " ".join(
                    "_" + name + "=_" + ("Yes" if value else "No")
                    for name, value in step["options"].items()
                )
                tail = {
                    "Complete": "5 90 _Enter",
                    "CancelStart": "_Cancel",
                    "CancelEnd": "5 _Cancel",
                }.get(step["finish"])
                if step["finish"] in ("CopyThenCancel", "CopyThenEnter"):
                    pending = " ".join(
                        "_" + name + "=_" + ("Yes" if value else "No")
                        for name, value in step["pending_options"].items()
                    )
                    tail = "5 90 " + pending + (
                        " _Cancel" if step["finish"] == "CopyThenCancel" else " _Enter"
                    )
                from number_token import number_token
                result = invoke("_Maelstrom w0,0,0 " + number_token(step.get("radius0",2.)) + " " + options + " " + tail, "Maelstrom")
                if (
                    step["finish"] in ("CancelStart", "CancelEnd")
                    and result["after"] != before
                ):
                    raise ValueError("cancelled Maelstrom preference step changed geometry")
            elif kind == "RememberCopyOptions":
                before = snapshot()
                result = invoke(
                    "_RememberCopyOptions _" + ("Yes" if step["enabled"] else "No"),
                    kind,
                )
            elif kind == "New":
                # The private session owns every object; suppress the save prompt.
                doc.Modified = False
                before = snapshot()
                result = invoke("_-New _None", "New")
                doc = Rhino.RhinoDoc.ActiveDoc
                if list(doc.Objects):
                    raise ValueError(
                        "Maelstrom preference New did not produce an empty document"
                    )
                ids.clear()
                setup()
            else:
                before = snapshot()
                result = invoke("_" + kind, kind)
            records.append(
                dict(
                    step=step,
                    query_before=before_query,
                    before=before,
                    result=result,
                    query_after=query(),
                )
            )
        return dict(records=records), 0
    finally:
        for obj in list(doc.Objects):
            doc.Objects.Delete(obj.Id, True)
        doc.ModelAbsoluteTolerance = saved_tolerance
