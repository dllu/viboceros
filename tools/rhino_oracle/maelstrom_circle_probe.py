"""Closed first-Circle getter recipes on owned Maelstrom point sources."""
import math
import re

OPTIONS={"Radius","Diameter","Circumference","Area","Vertical","Orientation","2Point","3Point",
         "Copy=Yes","Copy=No","Rigid=Yes","Rigid=No","ProjectOsnap=Yes","ProjectOsnap=No"}


def valid_token(value):
    return (value is None or type(value) in (int,float) and not math.isnan(value) and not math.isinf(value) and abs(value)<=1440
        or isinstance(value,list) and len(value)==3 and all(type(v) in (int,float) and not math.isnan(v) and not math.isinf(v) and abs(v)<=100 for v in value)
        or isinstance(value,str) and value in OPTIONS)


def validate(op):
    fields={"op","id","normal","origin","inputs","circle_inputs","target","degrees"}
    if (not isinstance(op,dict) or set(op)!=fields or op["op"]!="maelstrom_circle_command"
        or not isinstance(op["id"],str) or re.match(r"^[A-Za-z0-9_-]{1,80}\Z",op["id"]) is None
        or any(not isinstance(op[k],list) or len(op[k])!=3 or not valid_token(op[k]) for k in ("origin","normal"))
        or not any(op["normal"]) or not isinstance(op["inputs"],list) or not 1<=len(op["inputs"])<=20
        or any(not valid_token(v) for v in op["inputs"])
        or op["circle_inputs"] is not None and (not isinstance(op["circle_inputs"],list) or not 1<=len(op["circle_inputs"])<=16 or any(not valid_token(v) for v in op["circle_inputs"]))
        or type(op["target"]) not in (int,float) or not 0<=op["target"]<=100
        or not valid_token(op["degrees"]) or isinstance(op["degrees"],str) or op["degrees"] is None):
        raise ValueError("invalid bounded Maelstrom Circle recipe")


def request():
    c=[0.,0.,0.]; normal=[0.,0.,1.]
    recipes=[]
    def add(circle,origin=c,n=normal,angle=90.,tail=None):
        inputs=circle+(tail if tail is not None else ["Copy=No","Rigid=No",5.,angle])
        recipes.append(dict(op="maelstrom_circle_command",id="maelstrom-circle-"+str(len(recipes)),normal=n,origin=origin,
                            inputs=inputs,circle_inputs=circle,target=5.,degrees=angle))
    for option,value in [("Radius",2.),("Diameter",4.),("Circumference",4*math.pi),("Area",4*math.pi)]:add([c,option,value])
    for option in ["Diameter","Circumference","Area"]:add([c,option,[0.,2.,7.]])
    add([c,"Diameter","Radius",2.])
    add(["Vertical",c,[3.,4.,0.]])
    add(["Vertical",c,"Radius",2.,[3.,4.,0.]])
    add(["Vertical",c,[0.,3.,4.]])
    add(["Vertical",c,"Diameter",4.,[3.,4.,0.]])
    add(["Vertical",c,"Circumference",4*math.pi])
    add(["Vertical",c,"Area",4*math.pi])
    for option,value in [("Radius",2.),("Diameter",4.),("Circumference",4*math.pi),("Area",4*math.pi)]:
        add([c,"Orientation",[1.,2.,3.],option,value])
    add([c,"Orientation",[0.,0.,5.],"Radius",2.])
    add([c,"Orientation",[1.,2.,3.],[2.,1.,7.]])
    for pts in [([-2.,0.,0.],[2.,0.,0.]),([1.,2.,3.],[4.,6.,3.]),([1.,2.,3.],[4.,6.,8.])]:add(["2Point"]+list(pts))
    for pts in [([2.,0.,0.],[0.,2.,0.],[-2.,0.,0.]),([2.,0.,0.],[-2.,0.,0.],[0.,2.,0.]),([1.,0.,0.],[0.,1.,0.],[0.,0.,1.])]:add(["3Point"]+list(pts))
    add(["3Point",[-2.,0.,0.],[2.,0.,0.],"Radius",3.,[0.,1.,0.]])
    add(["3Point",[-2.,0.,0.],[2.,0.,0.],"Radius",[0.,4.,0.]])
    add(["Vertical",c,"Radius",2.,[3.,4.,1.]],n=[1.,2.,3.])
    add([c,"Orientation",[1.,0.,0.],"Radius",2.],n=[1.,2.,3.])
    add(["2Point",[-2.,1.,3.],[2.,3.,5.]],n=[1.,2.,3.])
    for circle in [["3Point",[2.,0.,0.],[0.,2.,0.],[-2.,0.,0.]],
                   ["Vertical",c,"Radius",2.,[3.,4.,0.]],
                   [c,"Orientation",[1.,2.,3.],"Radius",2.]]:
        add(circle,angle=[3.,5.,7.])
    add([c,"ProjectOsnap=No",[0.,2.,7.]])
    add([c,"ProjectOsnap=Yes",[0.,2.,7.]])
    # Size edits and acceptance update the remembered radius even after Cancel.
    for inputs in [[c,"Diameter",8.],[c,None],[c,"Circumference",12.],[c,None],[c,"Area",16.],[c,None]]:
        add(inputs,tail=[]);recipes[-1]["circle_inputs"]=None
    for op in recipes:validate(op)
    return dict(protocol_version=1,iterations=1,operations=recipes)


def cross_request():
    c=[0.,0.,0.]
    cases=[]
    for i,(inputs,circle) in enumerate([
        ([c,"Diameter",8.,"Copy=No","Rigid=No",5.,90.],[c,"Radius",3.]),
        ([c,None],None),
        ([c,"Radius",4.,"Copy=No","Rigid=No",5.,90.],[c,"Diameter",10.]),
        ([c,None],None),
    ]):
        cases.append(dict(op="maelstrom_circle_command",id="maelstrom-circle-cross-"+str(i),normal=[0.,0.,1.],origin=c,
            inputs=inputs,circle_inputs=circle,target=5.,degrees=90.))
    return dict(protocol_version=1,iterations=1,operations=cases)


def memory_request():
    c=[0.,0.,0.];cases=[]
    for option,value in [("Diameter",8.),("Circumference",12.),("Area",16.)]:
        for inputs,reference in [([c,option,value],None),([c,None,"Copy=No","Rigid=No",5.,90.],[c,option,value])]:
            cases.append(dict(op="maelstrom_circle_command",id="maelstrom-circle-memory-"+str(len(cases)),normal=[0.,0.,1.],origin=c,
                inputs=inputs,circle_inputs=reference,target=5.,degrees=90.))
    return dict(protocol_version=1,iterations=1,operations=cases)


def point_request():
    c=[0.,0.,0.]; cases=[]
    for prefix in ([c], ["Vertical",c], [c,"Orientation",[1.,2.,3.]]):
        for option in ("Circumference","Area"):
            circle=list(prefix)+[option,[2.,1.,7.]]
            cases.append(dict(op="maelstrom_circle_command",id="maelstrom-circle-point-"+str(len(cases)),normal=[0.,0.,1.],origin=c,
                inputs=circle+["Copy=No","Rigid=No",5.,90.],circle_inputs=circle,target=5.,degrees=90.))
    for circle in ([[5.,4.,1.],"Circumference",17.], [[5.,4.,1.],"Orientation",[6.,6.,4.],"Area",[2.,6.,9.]]):
        cases.append(dict(op="maelstrom_circle_command",id="maelstrom-circle-point-"+str(len(cases)),normal=[1.,2.,3.],origin=[4.,-2.,3.],
            inputs=circle+["Copy=No","Rigid=No",5.,90.],circle_inputs=circle,target=5.,degrees=90.))
    return dict(protocol_version=1,iterations=1,operations=cases)


def angle_request():
    c=[0.,0.,0.];cases=[]
    for normal,first,second in [
        ([0.,0.,1.],[1.,2.,3.],[4.,6.,8.]),
        ([0.,0.,1.],[4.,6.,8.],[1.,2.,3.]),
        ([1.,2.,3.],[-2.,1.,3.],[2.,3.,5.]),
        ([0.,0.,1.],[2.,0.,1.],[-2.,0.,3.]),
    ]:
        circle=["2Point",first,second];angle=[3.,5.,7.]
        cases.append(dict(op="maelstrom_circle_command",id="maelstrom-circle-angle-"+str(len(cases)),normal=normal,origin=c,
            inputs=circle+["Copy=No","Rigid=No",5.,angle],circle_inputs=circle,target=5.,degrees=angle))
    return dict(protocol_version=1,iterations=1,operations=cases)


def resolve_inputs(values, diameter):
    # Radius/Diameter are mutually exclusive getter options. Repeating the
    # current name runs Rhino's transparent measurement command instead.
    construction = values[0] if isinstance(values[0],str) else "Center"
    result=[]
    for value in values:
        if value in ("Radius","Diameter") and construction!="3Point":
            desired=value=="Diameter"
            if diameter==desired:continue
            diameter=desired
        result.append(value)
    return result,diameter


def run(op,host):
    from join_probe import observe_command
    from number_token import number_token
    validate(op)
    Rhino,System=host["Rhino"],host["System"]
    doc=Rhino.RhinoDoc.ActiveDoc;vp=doc.Views.ActiveView.ActiveViewport;saved=vp.GetConstructionPlane()
    if list(doc.Objects):raise ValueError("Circle getter requires empty owned document")
    ids=[]
    points=[[2.,1.,-2.],[2.,1.,0.],[2.,1.,5.],[2.,1.,12.],[5.,1.,3.],[-4.,3.,2.],[1.,-3.,4.],[0.,0.,0.]]
    def token(v):
        if v is None:return "_Enter"
        if isinstance(v,list):return "w"+",".join(number_token(x) for x in v)
        if isinstance(v,str):return "_"+v.replace("=","=_")
        return number_token(v)
    def snapshot():
        return [dict(source=ids.index(o.Id) if o.Id in ids else None,selected=bool(o.IsSelected(False)),point=host["_xyz"](o.Geometry.Location)) for o in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber)]
    try:
        plane=Rhino.Geometry.Plane(host["_point"](op["origin"]),Rhino.Geometry.Vector3d(*op["normal"]));vp.SetConstructionPlane(plane)
        serial=doc.BeginUndoRecord("Maelstrom Circle sources")
        try:
            ids=[doc.Objects.AddPoint(host["_point"](p)) for p in points]
            if any(i==System.Guid.Empty for i in ids):raise ValueError("Circle source insertion failed")
            for i in ids:doc.Objects.Select(i)
        finally:doc.EndUndoRecord(serial)
        before=snapshot();marker="Viboceros Maelstrom Circle "+str(System.Guid.NewGuid());Rhino.RhinoApp.WriteLine(marker)
        diameter=host.get("_viboceros_circle_diameter",False)
        circle_count=len(op["inputs"])-4 if "Copy=No" in op["inputs"] else len(op["inputs"])
        first,diameter=resolve_inputs(op["inputs"][:circle_count],diameter)
        tail=list(op["inputs"][circle_count:])
        if len(tail)>=2 and diameter:tail[-2]=op["target"]*2
        resolved=first+tail
        host["_viboceros_circle_diameter"]=diameter
        macro="_Maelstrom "+" ".join(token(v) for v in resolved)+" _Cancel"
        host["_record_progress"](op["id"]+" "+macro)
        success,after,events=observe_command(Rhino.Commands.Command,"Maelstrom",lambda:Rhino.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        history=Rhino.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        Rhino.RhinoApp.RunScript("_Undo",False);undo=snapshot();Rhino.RhinoApp.RunScript("_Redo",False);redo=snapshot()
        circle_record=None;sdk=[]
        if op["circle_inputs"] is not None:
            circle_inputs,circle_diameter=resolve_inputs(op["circle_inputs"],host.get("_viboceros_circle_command_diameter",False))
            host["_viboceros_circle_command_diameter"]=circle_diameter
            circle_macro="_Circle "+" ".join(token(v) for v in circle_inputs)+" _Cancel"
            Rhino.RhinoApp.RunScript(circle_macro,True)
            for obj in list(doc.Objects):
                if not isinstance(obj.Geometry,Rhino.Geometry.Curve):continue
                ok,circle=obj.Geometry.TryGetCircle()
                if not ok:raise ValueError("native Circle did not return an analytic circle")
                cp=circle.Plane
                circle_record=dict(origin=host["_xyz"](cp.Origin),x=host["_xyz"](cp.XAxis),y=host["_xyz"](cp.YAxis),normal=host["_xyz"](cp.Normal),radius=circle.Radius,seam=host["_xyz"](obj.Geometry.PointAtStart))
                if isinstance(op["degrees"],list):
                    d=host["_point"](op["degrees"])-cp.Origin;x=d*cp.XAxis;y=d*cp.YAxis
                    angle=math.atan(y/x)-math.pi if x<0 else math.atan2(y,x)
                else:angle=math.radians(op["degrees"])
                morph=Rhino.Geometry.Morphs.MaelstromSpaceMorph(cp,circle.Radius,op["target"],angle)
                try:sdk=[host["_xyz"](morph.MorphPoint(host["_point"](p))) for p in points]
                finally:morph.Dispose()
                break
        return dict(before=before,after=after,undo=undo,redo=redo,success=success,events=events,history=history,script_macro=macro,resolved_inputs=resolved,diameter=diameter,circle=circle_record,sdk_points=sdk),0
    finally:
        for o in list(doc.Objects):doc.Objects.Delete(o.Id,True)
        vp.SetConstructionPlane(saved.Plane)
