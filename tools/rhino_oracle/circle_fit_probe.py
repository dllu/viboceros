"""Bounded owned Circle FitPoints command and public fitter witnesses."""
import math
import re


def validate(op):
    if (not isinstance(op,dict) or set(op)!={"op","id","points"} or op["op"]!="circle_fit_points"
        or not isinstance(op["id"],str) or re.match(r"^[A-Za-z0-9_-]{1,80}\Z",op["id"]) is None
        or not isinstance(op["points"],list) or not 3<=len(op["points"])<=512
        or any(not isinstance(p,list) or len(p)!=3 or any(type(v) not in (int,float) or math.isnan(v) or math.isinf(v) or abs(v)>1e6 for v in p) for p in op["points"])):
        raise ValueError("invalid bounded Circle fit recipe")


def request():
    rows=[]
    def add(points):
        rows.append(dict(op="circle_fit_points",id="circle-fit-"+str(len(rows)),points=points))
    add([[2.,0.,0.],[0.,2.,0.],[-2.,0.,0.]])
    add([[2.,0.,0.],[-2.,0.,0.],[0.,2.,0.]])
    add([[1.,0.,0.],[0.,1.,0.],[0.,0.,1.]])
    for count in (4,8,16,64):
        add([[3.+2.*math.cos(i*(2*math.pi)/count),-4.+2.*math.sin(i*(2*math.pi)/count),5.] for i in range(count)])
    for arc in (.25,1.,3.,6.):
        add([[1.+3.*math.cos(arc*i/15),2.+3.*math.sin(arc*i/15),0.] for i in range(16)])
    for noise in (.001,.05,.2):
        add([[1.+(3.+noise*math.cos(3*i))*math.cos(i*.4),2.+(3.+noise*math.cos(3*i))*math.sin(i*.4),0.] for i in range(16)])
    for noise in (.01,.2,1.):
        add([[1.+3.*math.cos(i*.4),2.+3.*math.sin(i*.4),noise*math.sin(i*2.3)] for i in range(16)])
    pts=[[2.,0.,0.],[0.,2.,0.],[-2.,0.,0.],[0.,-2.,0.],[2.2,0.,0.]]
    add(pts);add(pts+[pts[0]]*5)
    add([[0.,0.,0.],[1.,0.,0.],[2.,0.,0.]])
    add([[0.,0.,0.]]*4)
    add([[0.,0.,0.],[1.,0.,0.],[2.,1e-8,0.],[3.,0.,0.]])
    for op in rows:validate(op)
    return dict(protocol_version=1,iterations=1,operations=rows)


def run(op,host):
    from join_probe import observe_command
    validate(op)
    Rhino,System=host["Rhino"],host["System"]
    doc=Rhino.RhinoDoc.ActiveDoc
    if list(doc.Objects):raise ValueError("fit probe requires empty owned document")
    ids=[]
    def circle_record(c):
        return dict(origin=host["_xyz"](c.Center),x=host["_xyz"](c.Plane.XAxis),y=host["_xyz"](c.Plane.YAxis),normal=host["_xyz"](c.Normal),radius=c.Radius)
    def snapshot():
        result=[]
        for o in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
            row=dict(source=ids.index(o.Id) if o.Id in ids else None,selected=bool(o.IsSelected(False)))
            if isinstance(o.Geometry,Rhino.Geometry.Point):row.update(kind="point",point=host["_xyz"](o.Geometry.Location))
            else:
                ok,c=o.Geometry.TryGetCircle()
                row.update(kind="circle",circle=circle_record(c) if ok else None,seam=host["_xyz"](o.Geometry.PointAtStart))
            result.append(row)
        return result
    try:
        points=[host["_point"](p) for p in op["points"]]
        ok,circle=Rhino.Geometry.Circle.TryFitCircleToPoints(points)
        sdk=circle_record(circle) if ok else None
        serial=doc.BeginUndoRecord("Circle fit sources")
        try:ids=[doc.Objects.AddPoint(p) for p in points]
        finally:doc.EndUndoRecord(serial)
        if any(i==System.Guid.Empty for i in ids):raise ValueError("fit point insertion failed")
        before=snapshot();marker="Viboceros Circle Fit "+str(System.Guid.NewGuid());Rhino.RhinoApp.WriteLine(marker)
        macro="_Circle _FitPoints _SelAll _Enter _Cancel"
        host["_record_progress"](op["id"]+" "+macro)
        success,after,events=observe_command(Rhino.Commands.Command,"Circle",lambda:Rhino.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        history=Rhino.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        Rhino.RhinoApp.RunScript("_Undo",False);undo=snapshot();Rhino.RhinoApp.RunScript("_Redo",False);redo=snapshot()
        return dict(sdk=sdk,before=before,after=after,undo=undo,redo=redo,events=events,history=history,success=success,script_macro=macro),0
    finally:
        for o in list(doc.Objects):doc.Objects.Delete(o.Id,True)
