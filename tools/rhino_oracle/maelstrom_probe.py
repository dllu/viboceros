"""Closed public MaelstromSpaceMorph point witnesses in an owned Rhino host."""
import math
import re


def validate(op):
    command = isinstance(op,dict) and op.get("op") == "maelstrom_command_points"
    extra = {"copy", "rigid", "grouped"} if command else set()
    if (not isinstance(op, dict)
            or set(op) != {"op", "id", "origin", "normal", "radius0", "radius1", "angle_radians", "points"} | extra
            or op["op"] not in ("maelstrom_points", "maelstrom_command_points")
            or not isinstance(op["id"], str)
            or re.match(r"^[A-Za-z0-9_-]{1,80}\Z", op["id"]) is None
            or not isinstance(op["points"], list) or not 1 <= len(op["points"]) <= 256):
        raise ValueError("invalid bounded Maelstrom recipe")
    if command and any(type(op[k]) is not bool for k in extra):
        raise ValueError("native Maelstrom commands require boolean flags")
    for value in [op["radius0"], op["radius1"], op["angle_radians"]]:
        if type(value) not in (int, float) or math.isnan(value) or math.isinf(value) or abs(value) > 1e6:
            raise ValueError("Maelstrom scalars must be bounded and finite")
    for point in [op["origin"], op["normal"]] + op["points"]:
        if (not isinstance(point, list) or len(point) != 3
                or any(type(v) not in (int, float) or math.isnan(v) or math.isinf(v) or abs(v) > 1e6 for v in point)):
            raise ValueError("Maelstrom vectors must be bounded and finite")
    if command and (min(op["radius0"],op["radius1"])<=0.01 or not any(op["normal"])):
        raise ValueError("native Maelstrom commands require positive radii and a nonzero normal")


def request():
    cases = []
    samples = [[r*x, r*y, z]
               for r in [0., .01, 1., 2., 2.1, 2.3, 2.75, 3.5, 4.25, 4.7, 4.9, 5., 6., 10.]
               for x,y in [(1.,0.),(0.,1.),(.6,.8)] for z in [-2.,0.,7.]]
    def add(**changes):
        op = dict(op="maelstrom_points", id="maelstrom-sdk-"+str(len(cases)),
                  origin=[0.,0.,0.], normal=[0.,0.,1.], radius0=2., radius1=5.,
                  angle_radians=math.pi/2, points=samples)
        op.update(changes)
        validate(op)
        cases.append(op)
    for radii in [(2.,5.),(5.,2.),(2.,2.),(0.,5.),(5.,0.),(0.,0.),(-2.,5.),(2.,-5.),(-2.,-5.),(1e-15,5.),(5.,1e-15)]:
        add(radius0=radii[0],radius1=radii[1])
    for angle in [0.,-math.pi/2,math.pi,math.tau,7*math.pi,1e-14]:
        add(angle_radians=angle)
    for origin,normal in [([1.,2.,3.],[0.,0.,1.]),([1.,2.,3.],[1.,2.,3.]),([0.,0.,0.],[0.,0.,-1.]),([0.,0.,0.],[1.,0.,0.]),([0.,0.,0.],[0.,0.,0.]),([0.,0.,0.],[0.,0.,1e-15])]:
        add(origin=origin,normal=normal)
    return dict(protocol_version=1,iterations=1,operations=cases)


def followup_request():
    base = request()["operations"][0]
    cases = []
    zero = 2.**-32
    for value in [math.nextafter(zero,0.),zero,math.nextafter(zero,math.inf)]:
        for change in [dict(radius0=value),dict(radius1=value)]:
            cases.append(dict(base,id="maelstrom-boundary-"+str(len(cases)),**change))
    for value in [math.nextafter(2.,math.inf),2.+1e-12,2.+1e-10,2.+1e-8]:
        cases.append(dict(base,id="maelstrom-equal-"+str(len(cases)),radius1=value))
    for value in [1e-300,math.ulp(0.)]:
        cases.append(dict(base,id="maelstrom-normal-"+str(len(cases)),normal=[0.,0.,value]))
    samples=[[r,1.,z] for r in [1.,2.,3.5,5.,6.] for z in [-2.,0.,7.]]
    changes=[{},dict(radius0=5.,radius1=2.),dict(radius0=2.,radius1=2.),dict(copy=True),dict(rigid=True,grouped=True),dict(origin=[1.,2.,3.],normal=[1.,2.,3.]),dict(angle_radians=-math.pi/2),dict(angle_radians=0.)]
    for change in changes:
        op=dict(base,op="maelstrom_command_points",id="maelstrom-command-"+str(len(cases)),points=samples,copy=False,rigid=False,grouped=False)
        op.update(change)
        validate(op)
        cases.append(op)
    return dict(protocol_version=1,iterations=1,operations=cases)


def profile_request():
    base=request()["operations"][0]
    cases=[]
    for radius in [1e-3,1.,2.,1e3,1e5]:
        for delta in [1e-10,1e-8,1e-7,1e-5,1e-3]:
            cases.append(dict(base,id="maelstrom-profile-"+str(len(cases)),radius0=radius,radius1=radius+delta,
                              points=[[radius*factor,0.,0.] for factor in [.25,1.,1.5,3.]]))
    return dict(protocol_version=1,iterations=1,operations=cases)


def threshold_request():
    base=request()["operations"][0]
    cases=[]
    epsilon=2.**-26
    for radius in [1e-3,1.,2.,1e3]:
        for sign in [-1.,1.]:
            for factor in [.99,1.,1.01,1.99,2.,2.01]:
                cases.append(dict(base,id="maelstrom-threshold-"+str(len(cases)),radius0=radius,radius1=radius*(1.+sign*epsilon*factor),
                                  points=[[radius*1.5,0.,0.]]))
    return dict(protocol_version=1,iterations=1,operations=cases)


def run(op, host, iterations):
    validate(op)
    if op["op"] == "maelstrom_command_points":
        return run_command(op,host)
    Rhino = host["Rhino"]
    plane = Rhino.Geometry.Plane(host["_point"](op["origin"]),
                                 Rhino.Geometry.Vector3d(*op["normal"]))
    morph = Rhino.Geometry.Morphs.MaelstromSpaceMorph(plane, float(op["radius0"]),
                                                    float(op["radius1"]), float(op["angle_radians"]))
    try:
        points = [host["_point"](p) for p in op["points"]]
        result, elapsed = host["_measure"](iterations, lambda: [host["_xyz"](morph.MorphPoint(p)) for p in points])
        return dict(valid=bool(morph.IsValid), points=result), elapsed
    finally:
        morph.Dispose()


def run_command(op,host):
    from join_probe import observe_command
    from number_token import number_token
    Rhino,System=host["Rhino"],host["System"]
    doc=Rhino.RhinoDoc.ActiveDoc
    if list(doc.Objects): raise ValueError("Maelstrom requires an empty owned document")
    vp=doc.Views.ActiveView.ActiveViewport
    original_plane=vp.ConstructionPlane()
    saved_tolerance=doc.ModelAbsoluteTolerance
    ids=[]
    def snapshot():
        return [dict(source=ids.index(o.Id) if o.Id in ids else None,selected=bool(o.IsSelected(False)),point=host["_xyz"](o.Geometry.Location))
                for o in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber)]
    try:
        doc.ModelAbsoluteTolerance=1e-5
        vp.SetConstructionPlane(Rhino.Geometry.Plane(host["_point"](op["origin"]),Rhino.Geometry.Vector3d(*op["normal"])))
        serial=doc.BeginUndoRecord("Owned Maelstrom sources")
        try:
            ids=[doc.Objects.AddPoint(host["_point"](p)) for p in op["points"]]
            if any(key==System.Guid.Empty for key in ids): raise ValueError("Maelstrom point insertion failed")
            if op["grouped"]: doc.Groups.Add("OwnedMaelstrom",ids)
            for key in ids: doc.Objects.Select(key)
        finally: doc.EndUndoRecord(serial)
        before=snapshot()
        options=" ".join("_"+name+"=_"+("Yes" if op[key] else "No") for name,key in [("Copy","copy"),("Rigid","rigid")])
        macro="_Maelstrom w"+",".join(number_token(v) for v in op["origin"])+" "+number_token(op["radius0"])+" "+options+" "+number_token(op["radius1"])+" "+number_token(math.degrees(op["angle_radians"]))+(" _Enter" if op["copy"] else "")
        marker="Viboceros Maelstrom "+str(System.Guid.NewGuid())
        Rhino.RhinoApp.WriteLine(marker)
        host["_record_progress"](op["id"]+" "+macro)
        success,after,events=observe_command(Rhino.Commands.Command,"Maelstrom",lambda:Rhino.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        history=Rhino.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        undo=redo=None
        if after!=before:
            Rhino.RhinoApp.RunScript("_Undo",False);undo=snapshot()
            Rhino.RhinoApp.RunScript("_Redo",False);redo=snapshot()
        return dict(success=success,before=before,after=after,undo=undo,redo=redo,events=events,history=history),0
    finally:
        for o in list(doc.Objects): doc.Objects.Delete(o.Id,True)
        for i in range(doc.Groups.Count):
            if not doc.Groups[i].IsDeleted: doc.Groups.Delete(i)
        vp.SetConstructionPlane(original_plane)
        doc.ModelAbsoluteTolerance=saved_tolerance
