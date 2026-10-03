"""Bounded public BendSpaceMorph maps and owned actual Bend point commands."""

import math
import re


def validate(op):
    common = {
        "op",
        "id",
        "start",
        "end",
        "through",
        "angle",
        "straight",
        "symmetric",
        "points",
    }
    kind = op.get("op") if isinstance(op, dict) else None
    extra = (
        {"copy", "rigid", "preserve", "non_attenuated", "grouped"}
        if kind == "bend_command_points"
        else set()
    )
    if (
        not isinstance(op, dict)
        or kind not in ("bend_points", "bend_command_points")
        or set(op) != common | extra
        or not isinstance(op["id"], str)
        or re.match(r"^[A-Za-z0-9_-]{1,80}\Z", op["id"]) is None
        or any(
            type(op[key]) is not bool
            for key in ("straight", "symmetric") + tuple(extra)
        )
        or not isinstance(op["points"], list)
        or not 1 <= len(op["points"]) <= 256
    ):
        raise ValueError("invalid bounded Bend recipe")
    if op["angle"] is not None and (
        type(op["angle"]) not in (int, float)
        or math.isnan(op["angle"])
        or math.isinf(op["angle"])
        or abs(op["angle"]) > 4 * math.pi
    ):
        raise ValueError("invalid Bend angle")
    for p in [op["start"], op["end"], op["through"]] + op["points"]:
        if (
            not isinstance(p, list)
            or len(p) != 3
            or any(
                type(v) not in (int, float)
                or math.isnan(v)
                or math.isinf(v)
                or abs(v) > 1e6
                for v in p
            )
        ):
            raise ValueError("Bend samples require bounded finite points")
    if op["start"] == op["end"]:
        raise ValueError("Bend spine points must differ")


def request():
    cases = []
    samples = [
        [x, 1.0, z]
        for x in [-2.0, 0.0, 2.0]
        for z in [-12.0, -5.0, -2.0, 0.0, 2.5, 5.0, 7.5, 10.0, 12.0, 15.0, 20.0]
    ]
    samples += [[0.0, 0.0, z] for z in [0.0, 2.5, 5.0, 7.5, 10.0]]

    def add(
        straight=True, symmetric=False, through=None, angle=None, start=None, end=None
    ):
        cases.append(
            dict(
                op="bend_points",
                id="bend-sdk-" + str(len(cases)),
                start=start or [0.0, 0.0, 0.0],
                end=end or [0.0, 0.0, 10.0],
                through=through or [10.0, 0.0, 10.0],
                angle=angle,
                straight=straight,
                symmetric=symmetric,
                points=samples,
            )
        )

    for straight in (False, True):
        for symmetric in (False, True):
            for angle in (None, math.pi / 2):
                add(straight, symmetric, angle=angle)
    for through in [
        [5.0, 0.0, 10.0],
        [2.0, 0.0, 5.0],
        [10.0, 0.0, 5.0],
        [-10.0, 0.0, 10.0],
    ]:
        for straight in (False, True):
            add(straight, through=through)
    for angle in [0.0, -math.pi / 2, math.pi / 4, math.pi, 2 * math.pi, 1e-8]:
        add(angle=angle)
    add(start=[0.0, 0.0, 10.0], end=[0.0, 0.0, 0.0], through=[10.0, 0.0, 0.0])
    add(
        start=[1.0, 2.0, 3.0],
        end=[5.0, 6.0, 11.0],
        through=[9.0, 6.0, 7.0],
        angle=math.pi / 3,
    )
    return dict(protocol_version=1, iterations=1, operations=cases)


def command_request():
    source = request()["operations"][2]
    cases = []
    for straight in (False, True):
        for symmetric in (False, True):
            for non_attenuated in (False, True):
                cases.append(
                    dict(
                        source,
                        op="bend_command_points",
                        id="bend-command-" + str(len(cases)),
                        straight=straight,
                        symmetric=symmetric,
                        non_attenuated=non_attenuated,
                        copy=False,
                        rigid=False,
                        preserve=False,
                        grouped=False,
                    )
                )
    return dict(protocol_version=1, iterations=1, operations=cases)


def edge_request():
    source = request()["operations"][0]
    samples = [
        [x, 1.0, z]
        for x in [-100.0, -2.0, 0.0, 2.0, 9.0, 9.99, 10.0, 10.01, 11.0, 20.0, 100.0]
        for z in [-20.0, -1.0, 0.0, 1.0, 5.0, 10.0, 15.0, 20.0]
    ]
    cases = [dict(source, id="bend-edge-radius", points=samples)]
    for i, through in enumerate(
        [
            [10.0, 0.0, -10.0],
            [10.0, 0.0, 0.0],
            [0.0, 0.0, 10.0],
            [0.0, 0.0, 0.0],
            [0.001, 0.0, 10.0],
        ]
    ):
        cases.append(dict(source, id="bend-edge-target-" + str(i), through=through))
    return dict(protocol_version=1, iterations=1, operations=cases)


def command_followup_request():
    source = command_request()["operations"][1]
    cases = []
    for through in [
        [5.0, 0.0, 10.0],
        [2.0, 0.0, 5.0],
        [10.0, 0.0, 5.0],
        [20.0, 0.0, 10.0],
        [5.0, 0.0, 5.0],
        [2.0, 0.0, 10.0],
        [0.1, 0.0, 10.0],
    ]:
        for straight in (False, True):
            cases.append(
                dict(
                    source,
                    id="bend-follow-" + str(len(cases)),
                    through=through,
                    straight=straight,
                )
            )
    for straight in (False, True):
        for angle in [math.pi / 4, math.pi / 2, math.pi, 0.0, -math.pi / 2]:
            cases.append(
                dict(
                    source,
                    id="bend-follow-" + str(len(cases)),
                    straight=straight,
                    angle=angle,
                )
            )
    cases.append(dict(source, id="bend-follow-copy", copy=True))
    cases.append(dict(source, id="bend-follow-rigid", rigid=True, grouped=True))
    return dict(protocol_version=1, iterations=1, operations=cases)


def angle_request():
    source = request()["operations"][0]
    cases = []
    for straight in (False, True):
        for angle in [3 * math.pi, 4 * math.pi, 1e-12, None]:
            cases.append(
                dict(
                    source,
                    id="bend-angle-sdk-" + str(len(cases)),
                    straight=straight,
                    angle=angle,
                    through=[0.01, 0.0, 0.0],
                )
            )
    return dict(protocol_version=1, iterations=1, operations=cases)


def angle_command_request():
    source = command_request()["operations"][1]
    cases = []
    for straight in (False, True):
        for angle in [math.pi / 4, math.pi / 2, math.pi, 0.0, -math.pi / 2]:
            cases.append(
                dict(
                    source,
                    id="bend-angle-command-" + str(len(cases)),
                    straight=straight,
                    angle=angle,
                )
            )
    return dict(protocol_version=1, iterations=1, operations=cases)


def boundary_request():
    source = request()["operations"][0]
    zero = 2.0**-32
    angles = [
        1e-10,
        zero,
        zero * (1 + 1e-6),
        2 * zero,
        2 * math.pi,
        2 * math.pi + 1e-10,
        2 * math.pi + 1e-9,
    ]
    cases = [
        dict(source, id="bend-boundary-" + str(i), angle=angle)
        for i, angle in enumerate(angles)
    ]
    return dict(protocol_version=1, iterations=1, operations=cases)


def small_angle_request():
    source = request()["operations"][0]
    cases = []
    for through in [[0.01, 0.0, 0.0], [10.0, 0.0, 10.0], [0.001, 0.0, 10.0]]:
        for angle in [1e-14, 1e-12, 1e-10, 1e-8]:
            cases.append(
                dict(
                    source,
                    id="bend-small-angle-" + str(len(cases)),
                    through=through,
                    angle=angle,
                )
            )
    return dict(protocol_version=1, iterations=1, operations=cases)


def short_arc_request():
    source = request()["operations"][0]
    cases = []
    zero = 2.0**-32
    for through, angles in [
        (
            [10.0, 0.0, 10.0],
            [1e-12, 1.000001e-12, 2e-12, 1e-11, zero / 10, zero / 10 * (1 + 1e-6)],
        ),
        ([0.001, 0.0, 10.0], [1e-12, 1.000001e-12, 2e-12, 1e-11]),
        ([0.01, 0.0, 0.0], [zero / 0.005, zero / 0.005 * (1 + 1e-6), 1e-7, 1e-6]),
    ]:
        for angle in angles:
            cases.append(
                dict(
                    source,
                    id="bend-short-arc-" + str(len(cases)),
                    through=through,
                    angle=angle,
                )
            )
    return dict(protocol_version=1, iterations=1, operations=cases)


def validity_request():
    source = request()["operations"][0]
    angular = (2.0**-32) * math.pi / 180
    angles = [3e-12, angular * (1 - 1e-6), angular, angular * (1 + 1e-6), 5e-12]
    cases = [
        dict(
            source,
            id="bend-validity-" + str(i),
            through=[0.001, 0.0, 10.0],
            angle=angle,
        )
        for i, angle in enumerate(angles)
    ]
    return dict(protocol_version=1, iterations=1, operations=cases)


def rigid_request():
    source = command_request()["operations"][0]
    cases = []
    for non_attenuated in (False, True):
        for offset in (0.0, 2.0):
            cases.append(
                dict(
                    source,
                    id="bend-rigid-" + str(len(cases)),
                    rigid=True,
                    grouped=True,
                    non_attenuated=non_attenuated,
                    points=[[p[0] + offset, p[1], p[2]] for p in source["points"]],
                )
            )
    return dict(protocol_version=1, iterations=1, operations=cases)


def run(op, host, iterations):
    validate(op)
    if op["op"] == "bend_command_points":
        return run_command(op, host)
    Rhino = host["Rhino"]
    args = [host["_point"](op[k]) for k in ("start", "end", "through")]
    if op["angle"] is not None:
        args.append(float(op["angle"]))
    args.extend([op["straight"], op["symmetric"]])
    morph = Rhino.Geometry.Morphs.BendSpaceMorph(*args)
    try:
        morph.QuickPreview = False
        points = [host["_point"](p) for p in op["points"]]
        result, elapsed = host["_measure"](
            iterations, lambda: [host["_xyz"](morph.MorphPoint(p)) for p in points]
        )
        return dict(valid=bool(morph.IsValid), points=result), elapsed
    finally:
        morph.Dispose()


def run_command(op, host):
    from join_probe import observe_command

    Rhino, System = host["Rhino"], host["System"]
    doc = Rhino.RhinoDoc.ActiveDoc
    if list(doc.Objects):
        raise ValueError("Bend commands require an empty owned document")
    saved_tolerance = doc.ModelAbsoluteTolerance
    ids = []

    def snapshot():
        objects = sorted(list(doc.Objects), key=lambda o: o.RuntimeSerialNumber)
        return [
            dict(
                source=ids.index(obj.Id) if obj.Id in ids else None,
                selected=bool(obj.IsSelected(False)),
                point=host["_xyz"](obj.Geometry.Location),
            )
            for obj in objects
        ]

    def point(p):
        return "w" + ",".join(str(float(v)) for v in p)

    try:
        doc.ModelAbsoluteTolerance = 1e-5
        serial = doc.BeginUndoRecord("Bend point sources")
        try:
            for p in op["points"]:
                ids.append(doc.Objects.AddPoint(host["_point"](p)))
            if any(key == System.Guid.Empty for key in ids):
                raise ValueError("Bend point insertion failed")
            if op["grouped"]:
                doc.Groups.Add("BendSource", ids)
            for key in ids:
                doc.Objects.Select(key)
        finally:
            doc.EndUndoRecord(serial)
        before = snapshot()
        options = " ".join(
            "_" + name + "=_" + ("Yes" if op[key] else "No")
            for name, key in [
                ("Copy", "copy"),
                ("Rigid", "rigid"),
                ("LimitToSpine", "straight"),
                ("Symmetric", "symmetric"),
                ("PreserveStructure", "preserve"),
                ("NonAttenuated", "non_attenuated"),
            ]
        )
        angle = (
            "" if op["angle"] is None else "_Angle " + str(math.degrees(op["angle"]))
        )
        macro = (
            "_Bend "
            + point(op["start"])
            + " "
            + point(op["end"])
            + " "
            + options
            + " "
            + angle
            + " "
            + point(op["through"])
            + " _Enter _Cancel"
        )
        marker = "Viboceros Bend " + str(System.Guid.NewGuid())
        Rhino.RhinoApp.WriteLine(marker)
        host["_record_progress"](op["id"] + " " + macro)
        success, after, events = observe_command(
            Rhino.Commands.Command,
            "Bend",
            lambda: Rhino.RhinoApp.RunScript(macro, True),
            snapshot,
            lambda: [],
            True,
        )
        history = Rhino.RhinoApp.CommandHistoryWindowText.split(marker, 1)[1].strip()
        undo = redo = None
        if [v["point"] for v in after] != [v["point"] for v in before]:
            Rhino.RhinoApp.RunScript("_Undo", False)
            undo = snapshot()
            Rhino.RhinoApp.RunScript("_Redo", False)
            redo = snapshot()
        return (
            dict(
                success=success,
                before=before,
                after=after,
                undo=undo,
                redo=redo,
                events=events,
                history=history,
            ),
            0,
        )
    finally:
        for obj in list(doc.Objects):
            doc.Objects.Delete(obj.Id, True)
        for i in range(doc.Groups.Count):
            if not doc.Groups[i].IsDeleted:
                doc.Groups.Delete(i)
        doc.ModelAbsoluteTolerance = saved_tolerance
