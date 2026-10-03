"""Bounded public TaperSpaceMorph maps and owned native point commands."""

import math
import re


def validate(op):
    common = {"op", "id", "start", "end", "start_radius", "end_radius",
              "flat", "infinite", "points"}
    extra = {"copy", "rigid", "preserve", "grouped"} if isinstance(op, dict) and op.get("op") == "taper_command_points" else set()
    if (not isinstance(op, dict) or op.get("op") not in ("taper_points", "taper_command_points")
            or set(op) != common | extra
            or not isinstance(op["id"], str) or re.match(r"^[A-Za-z0-9_-]{1,80}\Z", op["id"]) is None
            or any(type(op[k]) is not bool for k in ("flat", "infinite") + tuple(extra))
            or not isinstance(op["points"], list) or not 1 <= len(op["points"]) <= 256):
        raise ValueError("invalid bounded Taper recipe")
    for value in (op["start_radius"], op["end_radius"]):
        if type(value) not in (int, float) or math.isnan(value) or math.isinf(value) or abs(value) > 1e6:
            raise ValueError("Taper radii must be bounded finite numbers")
    for point in [op["start"], op["end"]] + op["points"]:
        if (not isinstance(point, list) or len(point) != 3
                or any(type(v) not in (int, float) or math.isnan(v) or math.isinf(v) or abs(v) > 1e6 for v in point)):
            raise ValueError("Taper points must be bounded and finite")


def request():
    cases = []
    samples = [[x, y, z] for x, y in [(2., 0.), (0., 3.), (2., 3.), (0., 0.)]
               for z in [-10., -1., 0., .1, 1., 2.5, 5., 7.5, 9., 9.9, 10., 11., 20.]]

    def add(**changes):
        op = dict(op="taper_points", id="taper-sdk-" + str(len(cases)),
                  start=[0., 0., 0.], end=[0., 0., 10.], start_radius=2., end_radius=1.,
                  flat=False, infinite=False, points=samples)
        op.update(changes)
        cases.append(op)

    for flat in (False, True):
        for infinite in (False, True):
            add(flat=flat, infinite=infinite)
    for radii in [(2., 4.), (2., 0.), (-2., 1.), (2., -1.), (0., 1.), (0., 0.), (2., 2.),
                  (1e-15, 1.), (1e-10, 1.), (1e-9, 1.), (1e-8, 1.)]:
        for infinite in (False, True):
            add(start_radius=radii[0], end_radius=radii[1], infinite=infinite)
    for start, end in [([0., 0., 0.], [10., 0., 0.]), ([0., 0., 0.], [0., 10., 0.]),
                       ([0., 0., 10.], [0., 0., 0.]), ([1., 2., 3.], [5., 6., 11.]),
                       ([0., 0., 0.], [0., 0., 0.]), ([0., 0., 0.], [0., 0., 1e-12])]:
        for flat in (False, True):
            add(start=start, end=end, flat=flat)
    return dict(protocol_version=1, iterations=1, operations=cases)


def command_request():
    cases = []
    base = request()["operations"][0]
    samples = [[2., 1., z] for z in [-2., 0., 2.5, 5., 7.5, 10., 12.]]
    for flat in (False, True):
        for infinite in (False, True):
            cases.append(dict(base, op="taper_command_points", id="taper-command-" + str(len(cases)),
                              points=samples, flat=flat, infinite=infinite,
                              copy=False, rigid=False, preserve=False, grouped=False))
    cases.append(dict(cases[0], id="taper-command-copy", copy=True))
    cases.append(dict(cases[0], id="taper-command-rigid", rigid=True, grouped=True))
    return dict(protocol_version=1, iterations=1, operations=cases)


def boundary_request():
    base = request()["operations"][0]
    cases = []
    zero = 2.0**-32
    for v in [zero*(1.-1e-6), zero, zero*(1.+1e-6)]:
        for changes in [dict(start_radius=v), dict(end_radius=v), dict(end=[0., 0., v])]:
            cases.append(dict(base, id="taper-boundary-"+str(len(cases)), **changes))
    for flat in (False, True):
        cases.append(dict(base, id="taper-crossing-"+str(len(cases)), flat=flat, infinite=True,
                          points=[[2.,3.,z] for z in [15.,20.,21.,30.,40.]]))
        cases.append(dict(base, id="taper-spatial-infinite-"+str(len(cases)), flat=flat, infinite=True,
                          start=[1.,2.,3.],end=[5.,6.,11.]))
    return dict(protocol_version=1,iterations=1,operations=cases)


def run(op, host, iterations):
    validate(op)
    if op["op"] == "taper_command_points":
        return run_command(op, host)
    Rhino = host["Rhino"]
    morph = Rhino.Geometry.Morphs.TaperSpaceMorph(host["_point"](op["start"]), host["_point"](op["end"]),
                                               float(op["start_radius"]), float(op["end_radius"]),
                                               op["flat"], op["infinite"])
    try:
        points = [host["_point"](p) for p in op["points"]]
        result, elapsed = host["_measure"](iterations, lambda: [host["_xyz"](morph.MorphPoint(p)) for p in points])
        return dict(valid=bool(morph.IsValid), points=result), elapsed
    finally:
        morph.Dispose()


def run_command(op, host):
    from join_probe import observe_command
    Rhino, System = host["Rhino"], host["System"]
    doc = Rhino.RhinoDoc.ActiveDoc
    if list(doc.Objects):
        raise ValueError("Taper requires an empty owned document")
    saved_tolerance = doc.ModelAbsoluteTolerance
    ids = []

    def snapshot():
        return [dict(source=ids.index(o.Id) if o.Id in ids else None, selected=bool(o.IsSelected(False)),
                     point=host["_xyz"](o.Geometry.Location))
                for o in sorted(list(doc.Objects), key=lambda o: o.RuntimeSerialNumber)]

    def point(p):
        return "w" + ",".join(str(float(v)) for v in p)

    try:
        doc.ModelAbsoluteTolerance = 1e-5
        serial = doc.BeginUndoRecord("Owned Taper sources")
        try:
            for p in op["points"]:
                ids.append(doc.Objects.AddPoint(host["_point"](p)))
            if any(key == System.Guid.Empty for key in ids):
                raise ValueError("Taper point insertion failed")
            if op["grouped"]:
                doc.Groups.Add("OwnedTaper", ids)
            for key in ids:
                doc.Objects.Select(key)
        finally:
            doc.EndUndoRecord(serial)
        before = snapshot()
        options = " ".join("_" + name + "=_" + ("Yes" if op[key] else "No")
                           for name, key in [("Copy", "copy"), ("Rigid", "rigid"), ("Flat", "flat"),
                                             ("Infinite", "infinite"), ("PreserveStructure", "preserve")])
        macro = "_Taper " + point(op["start"]) + " " + point(op["end"]) + " " + options + " " + str(float(op["start_radius"])) + " " + str(float(op["end_radius"])) + (" _Enter" if op["copy"] else "")
        marker = "Viboceros Taper " + str(System.Guid.NewGuid())
        Rhino.RhinoApp.WriteLine(marker)
        host["_record_progress"](op["id"] + " " + macro)
        success, after, events = observe_command(Rhino.Commands.Command, "Taper",
                                                lambda: Rhino.RhinoApp.RunScript(macro, True),
                                                snapshot, lambda: [], True)
        history = Rhino.RhinoApp.CommandHistoryWindowText.split(marker, 1)[1].strip()
        undo = redo = None
        if after != before:
            Rhino.RhinoApp.RunScript("_Undo", False)
            undo = snapshot()
            Rhino.RhinoApp.RunScript("_Redo", False)
            redo = snapshot()
        return dict(success=success, before=before, after=after, undo=undo, redo=redo,
                    events=events, history=history), 0
    finally:
        for o in list(doc.Objects):
            doc.Objects.Delete(o.Id, True)
        for i in range(doc.Groups.Count):
            if not doc.Groups[i].IsDeleted:
                doc.Groups.Delete(i)
        doc.ModelAbsoluteTolerance = saved_tolerance
