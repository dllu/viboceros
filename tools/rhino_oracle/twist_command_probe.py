"""Owned Twist commands with explicit presets and unmodified terminal snapshots."""

import math
import re

SHAPES = ("Points", "Line", "Curve", "Surface", "Box", "Mesh")


def validate(op):
    if (
        not isinstance(op, dict)
        or set(op) - {"axis", "offset_z", "tolerance", "angles"}
        != {
            "op",
            "id",
            "shape",
            "copy",
            "rigid",
            "preserve",
            "infinite",
            "degrees",
            "grouped",
        }
        or op["op"] != "twist_command"
        or not isinstance(op["id"], str)
        or re.match(r"^[A-Za-z0-9_-]{1,80}\Z", op["id"]) is None
        or op["shape"] not in SHAPES
        or any(
            type(op[k]) is not bool
            for k in ("copy", "rigid", "preserve", "infinite", "grouped")
        )
        or type(op["degrees"]) not in (int, float)
        or math.isnan(op["degrees"])
        or math.isinf(op["degrees"])
        or abs(op["degrees"]) > 1440.0
    ):
        raise ValueError("invalid bounded Twist command recipe")
    if (
        op.get("axis", "Z") not in ("Z", "LongZ", "Reverse", "Spatial")
        or type(op.get("offset_z", 0.0)) not in (int, float)
        or abs(op.get("offset_z", 0.0)) > 100
        or math.isnan(op.get("offset_z", 0.0))
        or type(op.get("tolerance", 1e-5)) not in (int, float)
        or not 1e-7 <= op.get("tolerance", 1e-5) <= 0.01
    ):
        raise ValueError("invalid Twist preset placement")
    if "angles" in op and (
        not op["copy"]
        or not isinstance(op["angles"], list)
        or not 1 <= len(op["angles"]) <= 8
        or any(
            type(a) not in (int, float)
            or math.isnan(a)
            or math.isinf(a)
            or abs(a) > 1440
            for a in op["angles"]
        )
    ):
        raise ValueError("invalid repeated Twist angles")


def request():
    cases = []

    def add(
        shape,
        copy=False,
        rigid=False,
        preserve=False,
        infinite=False,
        degrees=90.0,
        grouped=False,
    ):
        cases.append(
            dict(
                op="twist_command",
                id="twist-" + shape + "-" + str(len(cases)),
                shape=shape,
                copy=copy,
                rigid=rigid,
                preserve=preserve,
                infinite=infinite,
                degrees=degrees,
                grouped=grouped,
            )
        )

    for infinite in (False, True):
        for copy in (False, True):
            for rigid in (False, True):
                add("Points", copy, rigid, infinite=infinite, grouped=True)
    for shape in SHAPES[1:]:
        for infinite in (False, True):
            for preserve in (False, True):
                add(shape, preserve=preserve, infinite=infinite)
        add(shape, copy=True, rigid=True, grouped=True)
    add("Points", degrees=0.0)
    add("Points", degrees=-90.0)
    add("Points", degrees=720.0, infinite=True)
    return dict(protocol_version=1, iterations=1, operations=cases)


def rigid_request():
    r = request()
    source = r["operations"][17]
    ops = []
    for axis in ["Z", "LongZ", "Reverse", "Spatial"]:
        ops.append(dict(source, id="rigid-" + axis, axis=axis))
    for offset in [-4.0, -2.0, 2.0, 4.0]:
        ops.append(dict(source, id="rigid-offset-" + str(int(offset)), offset_z=offset))
    for tol in [1e-7, 0.01]:
        ops.append(dict(source, id="rigid-tol-" + str(len(ops)), tolerance=tol))
    ops.append(dict(r["operations"][1], id="rigid-ungrouped", grouped=False))
    ops.append(dict(r["operations"][33], id="zero-copy", copy=True, grouped=True))
    return dict(r, operations=ops)


def repeat_request():
    r = request()
    o = r["operations"][2]
    return dict(
        r,
        operations=[
            dict(o, id="repeat-" + str(i), angles=angles)
            for i, angles in enumerate([[180.0], [90.0], [-90.0], [0.0, 180.0]])
        ],
    )


def source_geometry(shape, host):
    Rhino = host["Rhino"]
    p = host["_point"]
    if shape == "Points":
        return [
            Rhino.Geometry.Point(p([2.0, 1.0, z]))
            for z in [-2.0, 0.0, 2.5, 5.0, 7.5, 10.0, 12.0]
        ]
    if shape == "Line":
        return [Rhino.Geometry.LineCurve(p([1.0, 0.0, -2.0]), p([3.0, 1.0, 12.0]))]
    if shape == "Curve":
        return [
            Rhino.Geometry.NurbsCurve.Create(
                False,
                3,
                [
                    p(v)
                    for v in (
                        [1.0, 0.0, -2.0],
                        [3.0, 2.0, 2.0],
                        [2.0, -1.0, 8.0],
                        [3.0, 1.0, 12.0],
                    )
                ],
            )
        ]
    if shape == "Surface":
        surface = Rhino.Geometry.NurbsSurface.CreateFromCorners(
            *[
                p(v)
                for v in (
                    [1.0, -1.0, -2.0],
                    [3.0, -1.0, -2.0],
                    [3.0, -1.0, 12.0],
                    [1.0, -1.0, 12.0],
                )
            ]
        )
        try:
            return [surface.ToBrep()]
        finally:
            surface.Dispose()
    if shape == "Box":
        return [
            Rhino.Geometry.Brep.CreateFromBox(
                Rhino.Geometry.BoundingBox(p([1.0, -1.0, 0.0]), p([3.0, 1.0, 10.0]))
            )
        ]
    mesh = Rhino.Geometry.Mesh()
    for v in ([1.0, -1.0, -2.0], [3.0, -1.0, -2.0], [3.0, 1.0, 12.0], [1.0, 1.0, 12.0]):
        mesh.Vertices.Add(*v)
    mesh.Faces.AddFace(0, 1, 2, 3)
    for i in range(4):
        mesh.VertexColors.Add(20 + i, 40 + i, 60 + i)
    mesh.Normals.ComputeNormals()
    return [mesh]


def geometry_record(g, host):
    Rhino = host["Rhino"]
    xyz = host["_xyz"]
    box = g.GetBoundingBox(True)
    value = dict(type=str(g.ObjectType), bounds=[xyz(box.Min), xyz(box.Max)])
    if isinstance(g, Rhino.Geometry.Point):
        value["points"] = [xyz(g.Location)]
    elif isinstance(g, Rhino.Geometry.Mesh):
        value["points"] = [xyz(p) for p in g.Vertices]
        value["faces"] = [[int(f.A), int(f.B), int(f.C), int(f.D)] for f in g.Faces]
        value["colors"] = [
            [int(c.R), int(c.G), int(c.B), int(c.A)] for c in g.VertexColors
        ]
    elif isinstance(g, Rhino.Geometry.Curve):
        value["definition"] = host["_nurbs_curve_definition"](g)
        value["samples"] = [
            xyz(g.PointAt(g.Domain.ParameterAt(i / 64.0))) for i in range(65)
        ]
    elif isinstance(g, Rhino.Geometry.Brep):
        value["vertices"] = [xyz(v.Location) for v in g.Vertices]
        value["solid"] = bool(g.IsSolid)
        value["orientation"] = str(g.SolidOrientation)
        value["surfaces"] = [host["_nurbs_surface_definition"](f) for f in g.Faces]
        value["samples"] = [
            xyz(
                f.PointAt(
                    f.Domain(0).ParameterAt(i / 8.0), f.Domain(1).ParameterAt(j / 8.0)
                )
            )
            for f in g.Faces
            for j in range(9)
            for i in range(9)
        ]
    else:
        raise ValueError("unexpected Twist output")
    return value


def run(op, host):
    validate(op)
    from join_probe import observe_command

    Rhino, System = host["Rhino"], host["System"]
    doc = Rhino.RhinoDoc.ActiveDoc
    if list(doc.Objects):
        raise ValueError("Twist capture requires empty owned document")
    ids = []
    owned = []
    saved_tolerance = doc.ModelAbsoluteTolerance

    def groups():
        return [i for i in range(doc.Groups.Count) if not doc.Groups[i].IsDeleted]

    def snapshot():
        objs = sorted(list(doc.Objects), key=lambda obj: obj.RuntimeSerialNumber)
        gs = groups()
        return dict(
            objects=[
                dict(
                    source=ids.index(obj.Id) if obj.Id in ids else None,
                    selected=bool(obj.IsSelected(False)),
                    name=obj.Attributes.Name,
                    color=[
                        int(obj.Attributes.ObjectColor.R),
                        int(obj.Attributes.ObjectColor.G),
                        int(obj.Attributes.ObjectColor.B),
                    ],
                    groups=[gs.index(i) for i in (obj.Attributes.GetGroupList() or [])],
                    geometry=geometry_record(obj.Geometry, host),
                )
                for obj in objs
            ],
            groups=[
                dict(
                    members=[
                        i
                        for i, obj in enumerate(objs)
                        if g in (obj.Attributes.GetGroupList() or [])
                    ]
                )
                for g in gs
            ],
        )

    try:
        doc.ModelAbsoluteTolerance = op.get("tolerance", 1e-5)
        serial = doc.BeginUndoRecord("Twist source setup")
        try:
            owned = source_geometry(op["shape"], host)
            if op.get("offset_z", 0.0):
                for g in owned:
                    g.Transform(
                        Rhino.Geometry.Transform.Translation(0.0, 0.0, op["offset_z"])
                    )
            for i, geom in enumerate(owned):
                attrs = Rhino.DocObjects.ObjectAttributes()
                attrs.Name = "source-" + str(i)
                attrs.ColorSource = Rhino.DocObjects.ObjectColorSource.ColorFromObject
                attrs.ObjectColor = System.Drawing.Color.FromArgb(10 + i, 30, 50)
                try:
                    ids.append(doc.Objects.Add(geom, attrs))
                finally:
                    attrs.Dispose()
            if any(key == System.Guid.Empty for key in ids):
                raise ValueError("Twist source insertion failed")
            if op["grouped"]:
                doc.Groups.Add("TwistSource", ids)
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
                ("Infinite", "infinite"),
            ]
        )
        # PreserveStructure is absent for polysurfaces; retain that command's measured shape policy.
        if op["shape"] != "Box":
            options += " _PreserveStructure=_" + ("Yes" if op["preserve"] else "No")
        axis = {
            "Z": "w0,0,0 w0,0,10",
            "LongZ": "w0,0,0 w0,0,20",
            "Reverse": "w0,0,10 w0,0,0",
            "Spatial": "w1,2,3 w5,6,11",
        }[op.get("axis", "Z")]
        macro = (
            "_Twist "
            + axis
            + " "
            + options
            + " "
            + " ".join(str(a) for a in [op["degrees"]] + op.get("angles", []))
            + " _Enter _Cancel"
        )
        marker = "Viboceros Twist " + str(System.Guid.NewGuid())
        Rhino.RhinoApp.WriteLine(marker)
        host["_record_progress"]("Twist " + op["id"] + " " + macro)
        success, after, events = observe_command(
            Rhino.Commands.Command,
            "Twist",
            lambda: Rhino.RhinoApp.RunScript(macro, False),
            snapshot,
            lambda: [],
            True,
        )
        history = Rhino.RhinoApp.CommandHistoryWindowText.split(marker, 1)[1].strip()
        undo = redo = None
        geometry_state = lambda state: dict(
            groups=state["groups"],
            objects=[
                dict((k, v) for k, v in obj.items() if k != "selected")
                for obj in state["objects"]
            ],
        )
        if geometry_state(after) != geometry_state(before):
            Rhino.RhinoApp.RunScript("_Undo", False)
            undo = snapshot()
            Rhino.RhinoApp.RunScript("_Redo", False)
            redo = snapshot()
        value = dict(
            before=before,
            after=after,
            undo=undo,
            redo=redo,
            success=success,
            events=events,
            history=history,
        )
        import json

        json.dumps(value, allow_nan=False)
        return value, 0
    finally:
        for obj in list(doc.Objects):
            doc.Objects.Delete(obj.Id, True)
        for g in groups():
            doc.Groups.Delete(g)
        for geom in owned:
            geom.Dispose()
        doc.ModelAbsoluteTolerance = saved_tolerance
