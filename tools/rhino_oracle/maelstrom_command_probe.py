"""Owned public Maelstrom commands, fitted geometry and atomic replay witnesses."""
import math
import re

if globals().get("__package__"):
    from .twist_command_probe import SHAPES, geometry_record, source_geometry
else:
    from twist_command_probe import SHAPES, geometry_record, source_geometry

FLAGS = (("Copy", "copy"), ("Rigid", "rigid"))


def scalar(value, bound=100):
    return type(value) in (int, float) and abs(value) <= bound and not math.isnan(value) and not math.isinf(value)


def vector(value):
    return isinstance(value, list) and len(value) == 3 and all(scalar(v) for v in value)


def radius(value):
    return scalar(value) or vector(value)


def unit(values):
    scale=max(abs(v) for v in values)
    values=[v/scale for v in values]
    length=math.hypot(math.hypot(values[0],values[1]),values[2])
    return [v/length for v in values]


def cross(a,b):
    return [a[(i+1)%3]*b[(i+2)%3]-a[(i+2)%3]*b[(i+1)%3] for i in range(3)]


def circle_normal(op):
    normal=unit(op["normal"])
    if not isinstance(op["radius0"],list):return normal
    direction=unit([op["radius0"][i]-op["origin"][i] for i in range(3)])
    projection=sum(a*b for a,b in zip(normal,direction))
    preferred=[normal[i]-projection*direction[i] for i in range(3)]
    if math.hypot(math.hypot(preferred[0],preferred[1]),preferred[2])>1e-12:
        return unit(preferred)
    # Public OpenNURBS-style plane axes, shared with the Rust Frame3 convention.
    x,y,z=[abs(v) for v in normal]
    if x<y:
        indices=(1,2,0) if x<z else (0,1,2)
    else:
        indices=(2,0,1) if z>x else ((0,2,1) if z>y else (0,1,2))
    first,second,zero=indices
    perpendicular=[0.,0.,0.]
    perpendicular[first]=-normal[second];perpendicular[second]=normal[first]
    return unit(cross(direction,cross(normal,unit(perpendicular))))


def validate(op):
    required = {"op", "id", "shape", "origin", "normal", "radius0", "radius1",
                "degrees", "copy", "rigid", "grouped", "finish"}
    if (not isinstance(op, dict) or set(op)-{"angles", "tolerance", "undo_unchanged", "diagnose_fit"} != required
            or op["op"] != "maelstrom_geometry_command" or not isinstance(op["id"], str)
            or re.match(r"^[A-Za-z0-9_-]{1,80}\Z", op["id"]) is None
            or op["shape"] not in SHAPES or not vector(op["origin"])
            or not vector(op["normal"]) or not any(op["normal"])
            or not radius(op["radius0"]) or not radius(op["radius1"])
            or not scalar(op["degrees"], 1440)
            or any(type(op[k]) is not bool for k in ("copy", "rigid", "grouped"))
            or op["finish"] not in ("Complete", "Cancel")
            or not scalar(op.get("tolerance", 1e-5), .01)
            or not 1e-12 <= op.get("tolerance", 1e-5) <= .01):
        raise ValueError("invalid bounded Maelstrom geometry recipe")
    if type(op.get("undo_unchanged", False)) is not bool:
        raise ValueError("invalid Maelstrom history policy")
    if type(op.get("diagnose_fit",False)) is not bool or (op.get("diagnose_fit") and
            (op["shape"]!="Box" or op["copy"] or op["rigid"] or any(type(op[k]) not in (int,float) or op[k]<=2.**-32 for k in ("radius0","radius1")))):
        raise ValueError("invalid Maelstrom fitting diagnostic")
    if "angles" in op and (not op["copy"] or not isinstance(op["angles"], list)
            or not 1 <= len(op["angles"]) <= 8
            or any(not scalar(v, 1440) for v in op["angles"])):
        raise ValueError("invalid repeated Maelstrom coil angles")
    first=op["radius0"]
    if isinstance(first,list):
        delta=[first[i]-op["origin"][i] for i in range(3)]
        first=math.hypot(math.hypot(delta[0],delta[1]),delta[2])
    # Rejected circle inputs must always have a terminal Cancel.
    if first <= 2.**-32 and op["finish"] != "Cancel":
        raise ValueError("potentially rejected Maelstrom radii require Cancel")
    if op.get("undo_unchanged",False) and (first <= 2.**-32 or op["finish"] != "Complete"):
        raise ValueError("unchanged-result history requires an accepted first radius")
    if isinstance(op["radius1"],list) and op["finish"]=="Complete":
        delta=[op["radius1"][i]-op["origin"][i] for i in range(3)]
        projected=cross(delta,circle_normal(op))
        if math.hypot(math.hypot(projected[0],projected[1]),projected[2])<=2.**-32:
            raise ValueError("rejected second radius points require Cancel")


def request():
    cases = []
    def add(shape="Points", **changes):
        op = dict(op="maelstrom_geometry_command", id="maelstrom-geometry-"+str(len(cases)),
                  shape=shape, origin=[0.,0.,0.], normal=[0.,0.,1.], radius0=2., radius1=5.,
                  degrees=90., copy=False, rigid=False, grouped=False, finish="Complete")
        op.update(changes)
        validate(op)
        cases.append(op)
    for shape in SHAPES:
        add(shape)
        add(shape, copy=True)
        add(shape, rigid=True)
        add(shape, copy=True, rigid=True, grouped=True)
        add(shape, degrees=0.)
    for change in [dict(radius0=5.,radius1=2.),dict(radius1=2.),dict(degrees=-90.),
                   dict(degrees=720.),dict(origin=[1.,2.,3.],normal=[1.,2.,3.]),
                   dict(normal=[0.,0.,-1.]),dict(normal=[1.,0.,0.]),
                   dict(radius0=[0.,2.,7.],radius1=[-5.,0.,9.]),
                   dict(copy=True,grouped=True,angles=[-90.,0.,180.]),
                   dict(copy=True,grouped=True,angles=[-90.],finish="Cancel"),
                   dict(radius0=-2.,finish="Cancel"),dict(radius1=-5.,finish="Cancel"),
                   dict(radius0=0.,finish="Cancel"),dict(radius1=0.,finish="Cancel"),
                   dict(radius0=2.**-32,finish="Cancel"),dict(radius1=2.**-32,finish="Cancel")]:
        add(**change)
    return dict(protocol_version=1, iterations=1, operations=cases)


def fitting_request():
    cases = []
    for op in request()["operations"]:
        if op["shape"] in ("Line", "Curve", "Surface", "Box") and not op["copy"] and not op["rigid"] and op["degrees"]:
            for tolerance in (1e-3, 1e-5, 1e-9):
                cases.append(dict(op, id="maelstrom-fitting-"+str(len(cases)), tolerance=tolerance))
    return dict(protocol_version=1, iterations=1, operations=cases)


def identity_request():
    cases = []
    for shape in SHAPES:
        for changes in [dict(degrees=0.), dict(radius1=0.), dict(degrees=0.,copy=True)]:
            base = next(op for op in request()["operations"] if op["shape"]==shape)
            cases.append(dict(base,id="maelstrom-identity-"+str(len(cases)),undo_unchanged=True,**changes))
    return dict(protocol_version=1,iterations=1,operations=cases)


def correspondence_request():
    base=next(op for op in request()["operations"] if op["shape"]=="Box")
    return dict(protocol_version=1,iterations=1,operations=[dict(base,id="maelstrom-box-fit-correspondence",diagnose_fit=True)])


def radius_request():
    base=request()["operations"][0]
    cases=[dict(radius0=[0.,1.,math.sqrt(3.)]),dict(radius0=[0.,0.,2.]),
           dict(radius0=[0.,2.,0.]),dict(radius0=[0.,0.,7.]),
           dict(radius1=[3.,0.,4.]),dict(radius1=[0.,0.,5.]),
           dict(radius1=[0.,0.,0.]),
           dict(origin=[1.,2.,3.],normal=[1.,2.,3.],radius0=[1.,3.,3.+math.sqrt(3.)])]
    operations=[dict(base,id="maelstrom-radius-point-"+str(i),finish="Cancel",**changes) for i,changes in enumerate(cases)]
    for op in operations:validate(op)
    return dict(protocol_version=1,iterations=1,operations=operations)


def run(op, host):
    from join_probe import observe_command

    validate(op)
    Rhino, System = host["Rhino"], host["System"]
    doc = Rhino.RhinoDoc.ActiveDoc
    if list(doc.Objects):
        raise ValueError("Maelstrom capture requires an empty owned document")
    ids, owned = [], []
    saved_tolerance = doc.ModelAbsoluteTolerance
    vp=doc.Views.ActiveView.ActiveViewport
    saved_cplane=vp.GetConstructionPlane()

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

    from number_token import number_token as scalar_token

    def point_token(point):
        return "w" + ",".join(scalar_token(v) for v in point)

    try:
        doc.ModelAbsoluteTolerance = op.get("tolerance", 1e-5)
        vp.SetConstructionPlane(Rhino.Geometry.Plane(host["_point"](op["origin"]), Rhino.Geometry.Vector3d(*op["normal"])))
        serial = doc.BeginUndoRecord("Maelstrom source setup")
        try:
            owned = source_geometry(op["shape"], host)
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
                raise ValueError("Maelstrom source insertion failed")
            if op["grouped"]:
                doc.Groups.Add("MaelstromSource", ids)
            for key in ids:
                doc.Objects.Select(key)
        finally:
            doc.EndUndoRecord(serial)
        before = snapshot()
        options = " ".join("_"+name+"=_"+("Yes" if op[key] else "No") for name,key in FLAGS)
        radius_token=lambda v: point_token(v) if isinstance(v,list) else scalar_token(v)
        macro = "_Maelstrom "+point_token(op["origin"])+" "+radius_token(op["radius0"])+" "+options+" "+radius_token(op["radius1"])+" "+" ".join(scalar_token(v) for v in [op["degrees"]]+op.get("angles",[]))
        macro += " _Cancel" if op["finish"]=="Cancel" else (" _Enter" if op["copy"] else "")
        marker = "Viboceros Maelstrom geometry " + str(System.Guid.NewGuid())
        Rhino.RhinoApp.WriteLine(marker)
        host["_record_progress"]("Maelstrom " + op["id"] + " " + macro)
        success, after, events = observe_command(
            Rhino.Commands.Command,
            "Maelstrom",
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

        if geometry_state(after) != geometry_state(before) or op.get("undo_unchanged",False):
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
            script_macro=macro,
        )
        if op.get("diagnose_fit",False):
            target=doc.Objects.FindId(ids[0]).Geometry
            source=owned[0]
            plane=Rhino.Geometry.Plane(host["_point"](op["origin"]),Rhino.Geometry.Vector3d(*op["normal"]))
            morph=Rhino.Geometry.Morphs.MaelstromSpaceMorph(plane,op["radius0"],op["radius1"],math.radians(op["degrees"]))
            samples=[]
            try:
                for face_index,(a,b) in enumerate(zip(source.Faces,target.Faces)):
                    for j in range(9):
                        for i in range(9):
                            u,v=a.Domain(0).ParameterAt(i/8.),a.Domain(1).ParameterAt(j/8.)
                            exact=morph.MorphPoint(a.PointAt(u,v))
                            fitted=b.PointAt(b.Domain(0).ParameterAt(i/8.),b.Domain(1).ParameterAt(j/8.))
                            found,cu,cv=b.ClosestPoint(exact)
                            if not found: raise ValueError("native Maelstrom fitting closest point failed")
                            closest=b.PointAt(cu,cv)
                            samples.append(dict(face=face_index,uv=[i/8.,j/8.],exact=host["_xyz"](exact),fitted=host["_xyz"](fitted),closest=host["_xyz"](closest),closest_uv=[cu,cv],relation=str(b.IsPointOnFace(cu,cv)),parameter_error=exact.DistanceTo(fitted),geometric_error=exact.DistanceTo(closest)))
            finally: morph.Dispose()
            value["fit_correspondence"]=samples
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
        vp.SetConstructionPlane(saved_cplane.Plane)
