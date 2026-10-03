"""Bounded, owned Bend commands with geometry, attributes and history snapshots."""

import math
import re

if globals().get("__package__"):
    from .twist_command_probe import SHAPES, geometry_record, source_geometry
else:
    from twist_command_probe import SHAPES, geometry_record, source_geometry

FLAGS = (
    ("Copy", "copy"),
    ("Rigid", "rigid"),
    ("LimitToSpine", "limit_to_spine"),
    ("Symmetric", "symmetric"),
    ("PreserveStructure", "preserve"),
    ("NonAttenuated", "non_attenuated"),
)
AXES = {
    "Z": "w0,0,0 w0,0,10",
    "Reverse": "w0,0,10 w0,0,0",
    "Spatial": "w1,2,3 w5,6,11",
}


def point_valid(value):
    return (
        isinstance(value, list)
        and len(value) == 3
        and all(
            type(v) in (int, float)
            and not math.isnan(v)
            and not math.isinf(v)
            and abs(v) <= 100
            for v in value
        )
    )


def validate(op):
    required = {"op", "id", "shape", "angle", "through", "grouped"} | {
        k for _, k in FLAGS
    }
    if (
        not isinstance(op, dict)
        or set(op) - {"axis", "tolerance", "targets", "offset_z"} != required
        or op["op"] != "bend_geometry_command"
        or not isinstance(op["id"], str)
        or re.match(r"^[A-Za-z0-9_-]{1,80}\Z", op["id"]) is None
        or op["shape"] not in SHAPES
        or any(type(op[k]) is not bool for k in ["grouped"] + [k for _, k in FLAGS])
        or not point_valid(op["through"])
        or (
            op["angle"] is not None
            and (
                type(op["angle"]) not in (int, float)
                or math.isnan(op["angle"])
                or math.isinf(op["angle"])
                or not 0 <= op["angle"] <= 360
            )
        )
        or not isinstance(op.get("axis", "Z"), str)
        or op.get("axis", "Z") not in AXES
        or type(op.get("tolerance", 1e-5)) not in (int, float)
        or not 1e-12 <= op.get("tolerance", 1e-5) <= 0.01
        or type(op.get("offset_z", 0.0)) not in (int, float)
        or math.isnan(op.get("offset_z", 0.0))
        or abs(op.get("offset_z", 0.0)) > 100
    ):
        raise ValueError("invalid bounded Bend geometry recipe")
    if "targets" in op and (
        not op["copy"]
        or not isinstance(op["targets"], list)
        or not 1 <= len(op["targets"]) <= 8
        or any(not point_valid(p) for p in op["targets"])
    ):
        raise ValueError("invalid repeated Bend targets")


def request():
    cases = []

    def add(shape, **options):
        op = dict(
            op="bend_geometry_command",
            id="bend-geometry-" + str(len(cases)),
            shape=shape,
            angle=None,
            through=[10.0, 0.0, 10.0],
            copy=False,
            rigid=False,
            limit_to_spine=True,
            symmetric=False,
            preserve=False,
            non_attenuated=False,
            grouped=False,
        )
        op.update(options)
        cases.append(op)

    for shape in SHAPES:
        for uniform in (False, True):
            for preserve in (False, True):
                add(shape, non_attenuated=uniform, preserve=preserve)
        add(shape, copy=True, rigid=True, grouped=True)
    for options in (
        dict(limit_to_spine=False),
        dict(symmetric=True),
        dict(angle=0.0),
        dict(angle=45.0),
        dict(angle=90.0),
        dict(angle=180.0),
        dict(angle=45.0, limit_to_spine=False),
        dict(through=[10.0, 0.0, -10.0]),
        dict(through=[-10.0, 0.0, 10.0]),
        dict(axis="Reverse"),
        dict(axis="Spatial"),
        dict(copy=True, grouped=True, targets=[[5.0, 0.0, 5.0]]),
    ):
        add("Points", **options)
    return dict(protocol_version=1, iterations=1, operations=cases)


def fitting_request():
    base = request()
    ops = []
    for shape in ("Line", "Curve", "Surface", "Box"):
        source = next(op for op in base["operations"] if op["shape"] == shape)
        for tolerance in (1e-3, 1e-5, 1e-9):
            ops.append(
                dict(source, id="bend-fitting-" + str(len(ops)), tolerance=tolerance)
            )
    return dict(base, operations=ops)


def rigid_request():
    base = request()
    source = base["operations"][4]
    cases = []
    for limited in (False, True):
        for offset in (-2.0, -1.0, 0.0, 1.0, 2.0):
            cases.append(
                dict(
                    source,
                    id="bend-limited-rigid-" + str(len(cases)),
                    limit_to_spine=limited,
                    offset_z=offset,
                )
            )
    return dict(base, operations=cases)


def rigid_midpoint_request():
    base = request()
    source = base["operations"][4]
    choices = [dict(offset_z=delta) for delta in (-1e-5, -1e-9, 1e-9, 1e-5)]
    choices += [
        dict(limit_to_spine=False, offset_z=2.5 * math.pi - 5.0),
        dict(axis="Reverse"),
        dict(axis="Spatial", offset_z=4.0),
        dict(offset_z=-10.0, symmetric=True),
    ]
    return dict(
        base,
        operations=[
            dict(source, id="bend-rigid-midpoint-" + str(i), **choice)
            for i, choice in enumerate(choices)
        ],
    )


def rigid_boundary_request():
    base = request()
    source = base["operations"][4]
    return dict(
        base,
        operations=[
            dict(source, id="bend-rigid-frame-boundary-" + str(i), offset_z=offset)
            for i, offset in enumerate((-1e-4, -3e-5, 3e-5, 1e-4))
        ],
    )


def run(op, host):
    from join_probe import observe_command

    validate(op)
    Rhino, System = host["Rhino"], host["System"]
    doc = Rhino.RhinoDoc.ActiveDoc
    if list(doc.Objects):
        raise ValueError("Bend capture requires an empty owned document")
    ids, owned = [], []
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

    def point_token(point):
        return "w" + ",".join(str(v) for v in point)

    try:
        doc.ModelAbsoluteTolerance = op.get("tolerance", 1e-5)
        serial = doc.BeginUndoRecord("Bend source setup")
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
                raise ValueError("Bend source insertion failed")
            if op["grouped"]:
                doc.Groups.Add("BendSource", ids)
            for key in ids:
                doc.Objects.Select(key)
        finally:
            doc.EndUndoRecord(serial)
        before = snapshot()
        # Native polysurfaces hide PreserveStructure. Set the other options
        # before Angle, because fixed-angle mode hides LimitToSpine as well.
        options = " ".join(
            "_" + name + "=_" + ("Yes" if op[key] else "No")
            for name, key in FLAGS
            if name != "PreserveStructure" or op["shape"] != "Box"
        )
        # Reset the remembered numeric angle explicitly for through-point cases.
        angle = " _Angle " + str(op["angle"] if op["angle"] is not None else 0)
        macro = (
            "_Bend "
            + AXES[op.get("axis", "Z")]
            + " "
            + options
            + angle
            + " "
            + " ".join(point_token(p) for p in [op["through"]] + op.get("targets", []))
            + " _Enter _Cancel"
        )
        marker = "Viboceros Bend geometry " + str(System.Guid.NewGuid())
        Rhino.RhinoApp.WriteLine(marker)
        host["_record_progress"]("Bend " + op["id"] + " " + macro)
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

        def geometry_state(state):
            return dict(
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
