"""Bounded, owned Bend preference workflows with actual prompt transcripts."""

import math
import re

OPTIONS = (
    "Copy",
    "Rigid",
    "LimitToSpine",
    "Symmetric",
    "PreserveStructure",
    "NonAttenuated",
)


def validate(op):
    if (
        not isinstance(op, dict)
        or set(op) != {"op", "id", "steps"}
        or op["op"] != "bend_options_command"
        or not isinstance(op["id"], str)
        or re.match(r"^[A-Za-z0-9_-]{1,80}\Z", op["id"]) is None
        or not isinstance(op["steps"], list)
        or not 1 <= len(op["steps"]) <= 64
    ):
        raise ValueError("invalid bounded Bend preferences workflow")
    for step in op["steps"]:
        if not isinstance(step, dict):
            raise ValueError("invalid Bend preferences step")
        kind = step.get("kind")
        if kind in ("Undo", "Redo", "New") and set(step) == {"kind"}:
            continue
        if (
            kind == "RememberCopyOptions"
            and set(step) == {"kind", "enabled"}
            and type(step["enabled"]) is bool
        ):
            continue
        if (
            kind != "Bend"
            or set(step) - {"pending_options"}
            != {"kind", "shape", "options", "finish", "degrees"}
            or step["shape"] not in ("Points", "Curve", "Box")
            or not isinstance(step["options"], dict)
            or set(step["options"]) - set(OPTIONS)
            or any(type(value) is not bool for value in step["options"].values())
            or (step["shape"] == "Box" and "PreserveStructure" in step["options"])
            or step["finish"]
            not in (
                "Complete",
                "CompleteThrough",
                "Cancel",
                "AngleCancel",
                "CopyThenCancel",
                "CopyThenEnter",
            )
            or type(step["degrees"]) not in (int, float)
            or math.isnan(step["degrees"])
            or math.isinf(step["degrees"])
            or not 0 <= step["degrees"] <= 360
        ):
            raise ValueError("invalid Bend preferences step")
        if step["finish"] in ("CopyThenCancel", "CopyThenEnter"):
            if (
                step["options"].get("Copy") is not True
                or not isinstance(step.get("pending_options"), dict)
                or set(step["pending_options"]) - set(OPTIONS)
                or any(
                    type(value) is not bool
                    for value in step["pending_options"].values()
                )
                or (
                    step["shape"] == "Box"
                    and "PreserveStructure" in step["pending_options"]
                )
            ):
                raise ValueError("invalid Bend pending Copy options")
        elif "pending_options" in step:
            raise ValueError("pending Bend options require a Copy placement")


def request():
    steps = []

    def bend(options=None, finish="Cancel", degrees=45, shape="Curve"):
        steps.append(
            dict(
                kind="Bend",
                shape=shape,
                options=options or {},
                finish=finish,
                degrees=degrees,
            )
        )

    bend()
    for name in OPTIONS[1:]:
        bend({name: True})
        bend()
    bend(
        {
            "Rigid": True,
            "LimitToSpine": False,
            "Symmetric": True,
            "PreserveStructure": True,
            "NonAttenuated": True,
        },
        "Complete",
        45,
    )
    steps.extend([dict(kind="Undo"), dict(kind="Redo")])
    bend(
        {
            "Rigid": False,
            "LimitToSpine": True,
            "Symmetric": False,
            "PreserveStructure": False,
            "NonAttenuated": False,
        },
        "AngleCancel",
        90,
    )
    bend({}, "Complete", 0)
    bend(
        {
            "Rigid": False,
            "LimitToSpine": True,
            "Symmetric": False,
            "NonAttenuated": False,
        },
        "Complete",
        45,
        "Box",
    )
    steps.append(dict(kind="RememberCopyOptions", enabled=False))
    bend({"Copy": True}, "Complete", 45)
    steps.append(dict(kind="RememberCopyOptions", enabled=True))
    bend({"Copy": True}, "Complete", 45)
    bend({"Copy": False}, "Cancel")
    steps.append(
        dict(
            kind="Bend",
            shape="Points",
            finish="CopyThenCancel",
            degrees=45,
            options={
                "Copy": True,
                "Rigid": False,
                "LimitToSpine": True,
                "Symmetric": False,
                "PreserveStructure": False,
                "NonAttenuated": False,
            },
            pending_options={
                "Rigid": True,
                "LimitToSpine": False,
                "Symmetric": True,
                "PreserveStructure": True,
                "NonAttenuated": True,
            },
        )
    )
    steps.extend([dict(kind="Undo"), dict(kind="Redo"), dict(kind="New")])
    bend()
    return dict(
        protocol_version=1,
        iterations=1,
        operations=[
            dict(op="bend_options_command", id="bend-preferences", steps=steps)
        ],
    )


def followup_request():
    base = request()
    steps = []

    def bend(finish="Complete", degrees=45, options=None, pending=None):
        step = dict(
            kind="Bend",
            shape="Curve",
            finish=finish,
            degrees=degrees,
            options=options or {},
        )
        if pending is not None:
            step["pending_options"] = pending
        steps.append(step)

    bend(
        options=dict(
            Copy=False, Rigid=False, PreserveStructure=True, LimitToSpine=False
        )
    )
    bend("CompleteThrough", options=dict(Copy=False, LimitToSpine=True))
    bend("AngleCancel", 90)
    bend("CompleteThrough")
    bend(degrees=0)
    bend("CompleteThrough", options=dict(LimitToSpine=False))
    steps.append(dict(kind="RememberCopyOptions", enabled=True))
    bend(
        "CopyThenCancel",
        options=dict(Copy=True, Rigid=True, PreserveStructure=False),
        pending=dict(Rigid=False, PreserveStructure=True),
    )
    steps.extend([dict(kind="Undo"), dict(kind="Redo")])
    bend(
        "CopyThenEnter",
        options=dict(Copy=True, Rigid=True, PreserveStructure=False),
        pending=dict(Rigid=False, PreserveStructure=True),
    )
    bend(options=dict(Copy=False, Rigid=False, PreserveStructure=True))
    steps.append(dict(kind="New"))
    bend("CompleteThrough")
    return dict(
        base,
        operations=[
            dict(op="bend_options_command", id="bend-preferences-followup", steps=steps)
        ],
    )


def run(op, host):
    from join_probe import observe_command
    from twist_command_probe import source_geometry, geometry_record

    validate(op)
    Rhino, System = host["Rhino"], host["System"]
    doc = Rhino.RhinoDoc.ActiveDoc
    if list(doc.Objects):
        raise ValueError("Bend preferences require an empty owned document")
    saved_tolerance = doc.ModelAbsoluteTolerance
    ids, records = {}, []

    def setup():
        doc.ModelAbsoluteTolerance = 1e-5
        serial = doc.BeginUndoRecord("Bend preferences source setup")
        try:
            for shape in ("Points", "Curve", "Box"):
                geometry = source_geometry(shape, host)
                try:
                    ids[shape] = [doc.Objects.Add(g) for g in geometry]
                finally:
                    for g in geometry:
                        g.Dispose()
                if any(key == System.Guid.Empty for key in ids[shape]):
                    raise ValueError("Bend preferences source insertion failed")
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
                raise ValueError("Bend preferences source selection failed")

    def invoke(macro, name):
        marker = "Viboceros Bend preferences " + str(System.Guid.NewGuid())
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
        result = invoke("_Bend w0,0,0 w0,0,10 _Cancel", "Bend")
        if result["after"] != before:
            raise ValueError("unedited Bend preference query changed its sources")
        matches = {
            name: re.findall(name + r"=(Yes|No)", result["history"]) for name in OPTIONS
        }
        if any(not values for values in matches.values()):
            raise ValueError(
                "Bend preference defaults missing from transcript: " + result["history"]
            )
        result["defaults"] = {
            name: values[0] == "Yes" for name, values in matches.items()
        }
        angle_query = invoke("_Bend w0,0,0 w0,0,10 _Angle _Cancel", "Bend")
        if angle_query["after"] != before:
            raise ValueError("Bend angle query changed its sources")
        matches = re.findall(r"Bend Angle <([^>]+)>", angle_query["history"])
        if not matches and "Bend Angle:" not in angle_query["history"]:
            raise ValueError("Bend angle prompt missing: " + angle_query["history"])
        result["angle_default"] = float(matches[0]) if matches else None
        result["angle_query"] = angle_query
        return result

    try:
        setup()
        for step in op["steps"]:
            before_query = query()
            kind = step["kind"]
            if kind == "Bend":
                select(step["shape"])
                before = snapshot()
                options = " ".join(
                    "_" + name + "=_" + ("Yes" if value else "No")
                    for name, value in step["options"].items()
                )
                tail = {
                    "Complete": "_Angle " + str(step["degrees"]) + " w10,0,10 _Enter",
                    "CompleteThrough": "w10,0,10 _Enter",
                    "Cancel": "_Cancel",
                    "AngleCancel": "_Angle " + str(step["degrees"]) + " _Cancel",
                }.get(step["finish"])
                if step["finish"] in ("CopyThenCancel", "CopyThenEnter"):
                    pending = " ".join(
                        "_" + name + "=_" + ("Yes" if value else "No")
                        for name, value in step["pending_options"].items()
                    )
                    tail = (
                        "_Angle "
                        + str(step["degrees"])
                        + " w10,0,10 _Angle 90 _Angle 0 "
                        + pending
                        + (
                            " _Cancel"
                            if step["finish"] == "CopyThenCancel"
                            else " _Enter"
                        )
                    )
                result = invoke("_Bend w0,0,0 w0,0,10 " + options + " " + tail, "Bend")
                if (
                    step["finish"] in ("Cancel", "AngleCancel")
                    and result["after"] != before
                ):
                    raise ValueError("cancelled Bend preference step changed geometry")
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
                        "Bend preference New did not produce an empty document"
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
