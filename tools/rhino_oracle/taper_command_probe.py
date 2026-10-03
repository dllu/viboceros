"""Owned Taper geometry commands, distance picks, attributes and replay."""
import math
import re

if globals().get("__package__"):
    from .twist_command_probe import SHAPES, geometry_record, source_geometry
else:
    from twist_command_probe import SHAPES, geometry_record, source_geometry

FLAGS = (("Copy", "copy"), ("Rigid", "rigid"), ("Flat", "flat"),
         ("Infinite", "infinite"), ("PreserveStructure", "preserve"))
AXES = {"Z": "w0,0,0 w0,0,10", "Reverse": "w0,0,10 w0,0,0", "Spatial": "w1,2,3 w5,6,11", "Skew": "w1,2,3 w4,-2,11", "X": "w0,0,0 w10,0,0", "Y": "w0,0,0 w0,10,0"}

def distance_valid(value):
    if type(value) in (int, float):
        return abs(value) <= 100 and not math.isnan(value) and not math.isinf(value)
    return isinstance(value,list) and len(value)==3 and all(type(v) in (int,float) and abs(v)<=100 and not math.isnan(v) and not math.isinf(v) for v in value)

def validate(op):
    required={"op","id","shape","start_distance","end_distance","finish","grouped"}|{k for _,k in FLAGS}
    if (not isinstance(op,dict) or set(op)-{"axis","tolerance","targets","cplane"}!=required
        or op["op"]!="taper_geometry_command" or not isinstance(op["id"],str)
        or re.match(r"^[A-Za-z0-9_-]{1,80}\Z",op["id"]) is None
        or op["shape"] not in SHAPES or any(type(op[k]) is not bool for k in ["grouped"]+[k for _,k in FLAGS])
        or not distance_valid(op["start_distance"]) or not distance_valid(op["end_distance"])
        or op["finish"] not in ("Complete","Cancel") or not isinstance(op.get("axis","Z"),str) or op.get("axis","Z") not in AXES
        or op.get("cplane","WorldXY") not in ("WorldXY","WorldYZ","WorldZX")
        or type(op.get("tolerance",1e-5)) not in (int,float) or not 1e-12<=op.get("tolerance",1e-5)<=.01):
        raise ValueError("invalid bounded Taper geometry recipe")
    if "targets" in op and (not op["copy"] or not isinstance(op["targets"],list)
        or not 1<=len(op["targets"])<=8 or any(not distance_valid(v) for v in op["targets"])):
        raise ValueError("invalid repeated Taper distances")

def request():
    cases=[]
    def add(shape="Points",**changes):
        op=dict(op="taper_geometry_command",id="taper-geometry-"+str(len(cases)),shape=shape,
                copy=False,rigid=False,flat=False,infinite=False,preserve=False,grouped=False,
                start_distance=2.,end_distance=1.,finish="Complete")
        op.update(changes);cases.append(op)
    for shape in SHAPES:
        add(shape);add(shape,flat=True);add(shape,preserve=True)
        add(shape,copy=True,rigid=True,grouped=True)
    for changes in [dict(infinite=True),dict(infinite=True,flat=True),dict(axis="Reverse"),dict(axis="Spatial"),
                    dict(axis="Spatial",flat=True),dict(axis="Spatial",rigid=True,grouped=True),
                    dict(start_distance=[0.,2.,0.],flat=True),dict(start_distance=[0.,2.,5.]),
                    dict(start_distance=[2.,0.,0.],end_distance=[-1.,0.,10.],flat=True),
                    dict(start_distance=[2.,0.,0.],end_distance=[1.,0.,0.],flat=True),
                    dict(end_distance=-1.,finish="Cancel"),dict(end_distance=0.,finish="Cancel"),
                    dict(start_distance=0.,finish="Cancel"),dict(copy=True,grouped=True,targets=[1.5,.5]),
                    dict(copy=True,grouped=True,targets=[1.5],finish="Cancel"),
                    dict(flat=True,cplane="WorldYZ"),dict(flat=True,cplane="WorldZX")]:
        add(**changes)
    return dict(protocol_version=1,iterations=1,operations=cases)

def fitting_request():
    cases=[]
    for op in request()["operations"]:
        if op["shape"] in ("Line","Curve","Surface","Box") and not any(op[k] for k in ("flat","preserve","rigid")):
            for tolerance in (1e-3,1e-5,1e-9):
                cases.append(dict(op,id="taper-fitting-"+str(len(cases)),tolerance=tolerance))
    return dict(protocol_version=1,iterations=1,operations=cases)

def followup_request():
    base = request()["operations"][0]
    changes = [
        dict(start_distance=-2., finish="Cancel"),
        dict(start_distance=-2., end_distance=-1., finish="Cancel"),
        dict(flat=True, start_distance=[2.,0.,0.], end_distance=[0.,1.,10.]),
        dict(flat=True, start_distance=[2.,0.,0.], end_distance=[1.,1.,10.]),
        dict(flat=True, axis="Skew"),
        dict(flat=True, axis="Skew", cplane="WorldYZ"),
        dict(flat=True, axis="Skew", cplane="WorldZX"),
        dict(flat=True, axis="X"),
        dict(flat=True, axis="Y"),
        dict(start_distance=1e-12, end_distance=1e-12, finish="Cancel"),
        dict(start_distance=1e-10, end_distance=1e-10, finish="Cancel"),
        dict(end_distance=1e-12, finish="Cancel"),
        dict(end_distance=-1e-12, finish="Cancel"),
        dict(start_distance=[0.,0.,5.], finish="Cancel"),
    ]
    return dict(protocol_version=1, iterations=1, operations=[
        dict(base, id="taper-followup-"+str(i), **change)
        for i, change in enumerate(changes)
    ])

def boundary_request():
    base=request()["operations"][0]
    changes=[dict(start_distance=-2.),dict(start_distance=-2.,end_distance=-1.),dict(end_distance=-1.)]
    for distance in (2.3283064365386963e-10,2.328306436538697e-10,1e-9):
        changes.extend([dict(start_distance=distance,end_distance=distance,finish="Cancel"),
                        dict(end_distance=distance,finish="Cancel")])
    return dict(protocol_version=1,iterations=1,operations=[
        dict(base,id="taper-command-boundary-"+str(i),**change) for i,change in enumerate(changes)
    ])

def threshold_request():
    base=request()["operations"][0]
    cases=[]
    for value in (2.3283064365386963e-10*(1.-1e-6), 2.3283064365386963e-10*(1.+1e-6), 3e-10, 5e-10):
        for change in (dict(start_distance=value,end_distance=value),dict(end_distance=value)):
            cases.append(dict(base,id="taper-command-threshold-"+str(len(cases)),finish="Cancel",**change))
    return dict(protocol_version=1,iterations=1,operations=cases)

def identity_request():
    return dict(protocol_version=1,iterations=1,operations=[
        dict(op,id="taper-identity-"+str(i),end_distance=2.)
        for i,op in enumerate(request()["operations"][::4][:6])
    ])

def run(op, host):
    from join_probe import observe_command

    validate(op)
    Rhino, System = host["Rhino"], host["System"]
    doc = Rhino.RhinoDoc.ActiveDoc
    if list(doc.Objects):
        raise ValueError("Taper capture requires an empty owned document")
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
        vp.SetConstructionPlane(getattr(Rhino.Geometry.Plane,op.get("cplane","WorldXY")))
        serial = doc.BeginUndoRecord("Taper source setup")
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
                raise ValueError("Taper source insertion failed")
            if op["grouped"]:
                doc.Groups.Add("TaperSource", ids)
            for key in ids:
                doc.Objects.Select(key)
        finally:
            doc.EndUndoRecord(serial)
        before = snapshot()
        options = " ".join("_"+name+"=_"+("Yes" if op[key] else "No")
                           for name,key in FLAGS if name!="PreserveStructure" or op["shape"]!="Box")
        distance=lambda v: point_token(v) if isinstance(v,list) else scalar_token(v)
        macro = "_Taper "+AXES[op.get("axis","Z")]+" "+options+" "+distance(op["start_distance"])+" "+" ".join(distance(v) for v in [op["end_distance"]]+op.get("targets",[]))
        macro += " _Cancel" if op["finish"]=="Cancel" else (" _Enter" if op["copy"] else "")
        marker = "Viboceros Taper geometry " + str(System.Guid.NewGuid())
        Rhino.RhinoApp.WriteLine(marker)
        host["_record_progress"]("Taper " + op["id"] + " " + macro)
        success, after, events = observe_command(
            Rhino.Commands.Command,
            "Taper",
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
            script_macro=macro,
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
        vp.SetConstructionPlane(saved_cplane.Plane)
