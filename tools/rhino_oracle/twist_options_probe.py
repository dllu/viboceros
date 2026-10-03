"""Bounded, owned Twist preference workflows with actual prompt transcripts."""

import math
import re

OPTIONS = ("Copy", "Rigid", "Infinite", "PreserveStructure")


def validate(op):
    if (
        not isinstance(op, dict)
        or set(op) != {"op", "id", "steps"}
        or op["op"] != "twist_options_command"
        or not isinstance(op["id"], str)
        or re.match(r"^[A-Za-z0-9_-]{1,80}\Z", op["id"]) is None
        or not isinstance(op["steps"], list)
        or not 1 <= len(op["steps"]) <= 64
    ):
        raise ValueError("invalid bounded Twist preferences workflow")
    for step in op["steps"]:
        if not isinstance(step, dict):
            raise ValueError("invalid Twist preferences step")
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
            kind != "Twist"
            or set(step) - {"pending_options"}
            != {"kind", "shape", "options", "finish", "degrees"}
            or step["shape"] not in ("Points", "Curve", "Box")
            or not isinstance(step["options"], dict)
            or set(step["options"]) - set(OPTIONS)
            or any(type(value) is not bool for value in step["options"].values())
            or (step["shape"] == "Box" and "PreserveStructure" in step["options"])
            or step["finish"]
            not in ("Complete", "Cancel", "ReferenceCancel", "CopyThenCancel")
            or type(step["degrees"]) not in (int, float)
            or math.isnan(step["degrees"])
            or math.isinf(step["degrees"])
            or abs(step["degrees"]) > 1440
        ):
            raise ValueError("invalid Twist preferences step")
        if step["finish"] == "CopyThenCancel":
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
                raise ValueError("invalid Twist pending Copy options")
        elif "pending_options" in step:
            raise ValueError("pending Twist options require a Copy placement")


def request():
    steps = []

    def twist(options=None, finish="Cancel", degrees=90, shape="Curve"):
        steps.append(
            dict(
                kind="Twist",
                shape=shape,
                options=options or {},
                finish=finish,
                degrees=degrees,
            )
        )

    twist()
    for name in ("Rigid", "Infinite", "PreserveStructure"):
        twist({name: True})
        twist()
        twist({name: False}, "Complete", 0)
        twist()
    twist({"Rigid": True, "Infinite": True, "PreserveStructure": True}, "Complete")
    steps.extend([dict(kind="Undo"), dict(kind="Redo")])
    twist()
    twist(
        {"Rigid": False, "Infinite": False, "PreserveStructure": False},
        "ReferenceCancel",
    )
    twist()
    twist({"PreserveStructure": True}, "Complete", 0)
    twist(shape="Box")
    twist({"Rigid": True, "Infinite": True}, "Cancel", shape="Box")
    twist(shape="Points")
    steps.append(dict(kind="RememberCopyOptions", enabled=False))
    twist({"Rigid": False, "Infinite": False}, "Complete", 0)
    twist({"Copy": True}, "Cancel")
    twist()
    steps.append(dict(kind="RememberCopyOptions", enabled=True))
    twist({"Copy": True}, "Complete", 45)
    twist()
    twist({"Copy": False}, "Cancel")
    twist()
    steps.append(dict(kind="New"))
    twist()
    steps.append(
        dict(
            kind="Twist",
            shape="Points",
            finish="CopyThenCancel",
            degrees=90,
            options={
                "Copy": True,
                "Rigid": False,
                "Infinite": False,
                "PreserveStructure": False,
            },
            pending_options={
                "Rigid": True,
                "Infinite": True,
                "PreserveStructure": True,
            },
        )
    )
    twist()
    steps.extend([dict(kind="Undo"), dict(kind="Redo")])
    twist()
    return dict(
        protocol_version=1,
        iterations=1,
        operations=[
            dict(op="twist_options_command", id="twist-preferences", steps=steps)
        ],
    )


def run(op, host):
    from join_probe import observe_command
    from twist_command_probe import source_geometry, geometry_record

    validate(op)
    Rhino, System = host["Rhino"], host["System"]
    doc = Rhino.RhinoDoc.ActiveDoc
    if list(doc.Objects):
        raise ValueError("Twist preferences require an empty owned document")
    saved_tolerance = doc.ModelAbsoluteTolerance
    ids, records = {}, []

    def setup():
        doc.ModelAbsoluteTolerance = 1e-5
        serial = doc.BeginUndoRecord("Twist preferences source setup")
        try:
            for shape in ("Points", "Curve", "Box"):
                geometry = source_geometry(shape, host)
                try:
                    ids[shape] = [doc.Objects.Add(g) for g in geometry]
                finally:
                    for g in geometry:
                        g.Dispose()
                if any(key == System.Guid.Empty for key in ids[shape]):
                    raise ValueError("Twist preferences source insertion failed")
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
                raise ValueError("Twist preferences source selection failed")

    def invoke(macro, name):
        marker = "Viboceros Twist preferences " + str(System.Guid.NewGuid())
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
        result = invoke("_Twist w0,0,0 w0,0,10 _Cancel", "Twist")
        if result["after"] != before:
            raise ValueError("unedited Twist preference query changed its sources")
        matches = {
            name: re.findall(name + r"=(Yes|No)", result["history"]) for name in OPTIONS
        }
        if any(not values for values in matches.values()):
            raise ValueError(
                "Twist preference defaults missing from transcript: "
                + result["history"]
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
            if kind == "Twist":
                select(step["shape"])
                before = snapshot()
                options = " ".join(
                    "_" + name + "=_" + ("Yes" if value else "No")
                    for name, value in step["options"].items()
                )
                tail = {
                    "Complete": str(step["degrees"]) + " _Enter",
                    "Cancel": "_Cancel",
                    "ReferenceCancel": "w1,0,0 _Cancel",
                }.get(step["finish"])
                if step["finish"] == "CopyThenCancel":
                    pending = " ".join(
                        "_" + name + "=_" + ("Yes" if value else "No")
                        for name, value in step["pending_options"].items()
                    )
                    tail = str(step["degrees"]) + " " + pending + " _Cancel"
                result = invoke(
                    "_Twist w0,0,0 w0,0,10 " + options + " " + tail, "Twist"
                )
                if (
                    step["finish"] in ("Cancel", "ReferenceCancel")
                    and result["after"] != before
                ):
                    raise ValueError("cancelled Twist preference step changed geometry")
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
                        "Twist preference New did not produce an empty document"
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
