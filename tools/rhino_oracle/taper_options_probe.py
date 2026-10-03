"""Owned, bounded Taper option edits, cancellation and remembered defaults."""
import re

OPTIONS = ("Copy", "Rigid", "Flat", "Infinite", "PreserveStructure")


def validate(op):
    if (not isinstance(op, dict) or set(op) != {"op", "id", "steps"}
        or op["op"] != "taper_options_command" or not isinstance(op["id"], str)
        or re.match(r"^[A-Za-z0-9_-]{1,80}\Z", op["id"]) is None
        or not isinstance(op["steps"], list) or not 1 <= len(op["steps"]) <= 64):
        raise ValueError("invalid bounded Taper preferences workflow")
    for step in op["steps"]:
        if not isinstance(step, dict):
            raise ValueError("invalid Taper preference step")
        if step.get("kind") in ("Undo", "Redo", "New") and set(step)=={"kind"}:
            continue
        if (step.get("kind")=="RememberCopyOptions" and set(step)=={"kind","enabled"}
            and type(step["enabled"]) is bool):
            continue
        if (step.get("kind") != "Taper" or set(step)-{"pending_options"} != {"kind","shape","options","finish"}
            or step["shape"] not in ("Points","Curve","Box")
            or step["finish"] not in ("Complete","CancelStart","CancelEnd","CopyThenCancel","CopyThenEnter")):
            raise ValueError("invalid Taper command step")
        for key in ("options", "pending_options"):
            if key not in step:
                continue
            if (not isinstance(step[key],dict) or set(step[key])-set(OPTIONS)
                or any(type(v) is not bool for v in step[key].values())
                or (step["shape"]=="Box" and "PreserveStructure" in step[key])):
                raise ValueError("invalid Taper option values")
        if step["finish"].startswith("CopyThen"):
            if step["options"].get("Copy") is not True or "pending_options" not in step:
                raise ValueError("Taper Copy placement requires pending options")
        elif "pending_options" in step:
            raise ValueError("pending Taper options require a Copy placement")


def request():
    steps=[]
    def taper(options=None, finish="CancelStart", shape="Curve", pending=None):
        step=dict(kind="Taper",shape=shape,options=options or {},finish=finish)
        if pending is not None:
            step["pending_options"]=pending
        steps.append(step)
    taper()
    for name in OPTIONS[1:]:
        taper({name:True})
        taper()
        taper({name:True}, "CancelEnd")
        taper()
    taper(dict(Copy=False,Rigid=True,Flat=True,Infinite=True,PreserveStructure=True),"Complete")
    steps.extend([dict(kind="Undo"),dict(kind="Redo")])
    taper(dict(Copy=False,Rigid=False,Flat=False,Infinite=False,PreserveStructure=False),"CancelEnd")
    taper(dict(Copy=False,Rigid=False,Flat=False,Infinite=False),"Complete","Box")
    steps.append(dict(kind="RememberCopyOptions",enabled=False))
    taper(dict(Copy=True),"Complete")
    steps.append(dict(kind="RememberCopyOptions",enabled=True))
    taper(dict(Copy=True),"Complete")
    taper(dict(Copy=False))
    taper(dict(Copy=True,Rigid=False,Flat=False,Infinite=False,PreserveStructure=False),
          "CopyThenCancel","Points",dict(Rigid=True,Flat=True,Infinite=True,PreserveStructure=True))
    steps.extend([dict(kind="Undo"),dict(kind="Redo")])
    taper(dict(Copy=True,Rigid=False,Flat=False,Infinite=False,PreserveStructure=False),
          "CopyThenEnter","Points",dict(Rigid=True,Flat=True,Infinite=True,PreserveStructure=True))
    steps.append(dict(kind="New"))
    taper()
    return dict(protocol_version=1,iterations=1,operations=[dict(op="taper_options_command",id="taper-preferences",steps=steps)])

def run(op, host):
    from join_probe import observe_command
    from twist_command_probe import source_geometry, geometry_record

    validate(op)
    Rhino, System = host["Rhino"], host["System"]
    doc = Rhino.RhinoDoc.ActiveDoc
    if list(doc.Objects):
        raise ValueError("Taper preferences require an empty owned document")
    saved_tolerance = doc.ModelAbsoluteTolerance
    ids, records = {}, []

    def setup():
        doc.ModelAbsoluteTolerance = 1e-5
        serial = doc.BeginUndoRecord("Taper preferences source setup")
        try:
            for shape in ("Points", "Curve", "Box"):
                geometry = source_geometry(shape, host)
                try:
                    ids[shape] = [doc.Objects.Add(g) for g in geometry]
                finally:
                    for g in geometry:
                        g.Dispose()
                if any(key == System.Guid.Empty for key in ids[shape]):
                    raise ValueError("Taper preferences source insertion failed")
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
                raise ValueError("Taper preferences source selection failed")

    def invoke(macro, name):
        marker = "Viboceros Taper preferences " + str(System.Guid.NewGuid())
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
        result = invoke("_Taper w0,0,0 w0,0,10 _Cancel", "Taper")
        if result["after"] != before:
            raise ValueError("unedited Taper preference query changed its sources")
        matches = {
            name: re.findall(name + r"=(Yes|No)", result["history"]) for name in OPTIONS
        }
        if any(not values for values in matches.values()):
            raise ValueError(
                "Taper preference defaults missing from transcript: " + result["history"]
            )
        result["defaults"] = {
            name: values[0] == "Yes" for name, values in matches.items()
        }
        return result

    try:
        setup()
        for step in op["steps"]:
            before_query = query()
            kind = step["kind"]
            if kind == "Taper":
                select(step["shape"])
                before = snapshot()
                options = " ".join(
                    "_" + name + "=_" + ("Yes" if value else "No")
                    for name, value in step["options"].items()
                )
                tail = {
                    "Complete": "2 1 _Enter",
                    "CancelStart": "_Cancel",
                    "CancelEnd": "2 _Cancel",
                }.get(step["finish"])
                if step["finish"] in ("CopyThenCancel", "CopyThenEnter"):
                    pending = " ".join(
                        "_" + name + "=_" + ("Yes" if value else "No")
                        for name, value in step["pending_options"].items()
                    )
                    tail = "2 1 " + pending + (
                        " _Cancel" if step["finish"] == "CopyThenCancel" else " _Enter"
                    )
                result = invoke("_Taper w0,0,0 w0,0,10 " + options + " " + tail, "Taper")
                if (
                    step["finish"] in ("CancelStart", "CancelEnd")
                    and result["after"] != before
                ):
                    raise ValueError("cancelled Taper preference step changed geometry")
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
                        "Taper preference New did not produce an empty document"
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
