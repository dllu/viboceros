"""Owned Maelstrom FitPoints commands, independent Circle frames and SDK maps."""
import math
import re


def vector(v):
    return (isinstance(v,list) and len(v)==3 and
            all(type(x) in (int,float) and not math.isnan(x) and not math.isinf(x) and abs(x)<=100 for x in v))


def validate(op):
    if (not isinstance(op,dict) or set(op)!={"op","id","points","normal","target","degrees","target_kind"}
        or op["op"]!="maelstrom_fit_points_command"
        or not isinstance(op["id"],str) or re.match(r"^[A-Za-z0-9_-]{1,80}\Z",op["id"]) is None
        or not isinstance(op["points"],list) or not 3<=len(op["points"])<=64
        or any(not vector(p) for p in op["points"]) or not vector(op["normal"]) or not any(op["normal"])
        or op["target_kind"] not in ("points","lines")
        or type(op["target"]) not in (int,float) or not 0<op["target"]<=100
        or type(op["degrees"]) not in (int,float) or math.isnan(op["degrees"]) or math.isinf(op["degrees"]) or abs(op["degrees"])>720):
        raise ValueError("invalid bounded Maelstrom FitPoints command")


def request():
    from .circle_fit_diagnostics import request as circle_request
    rows=[];circles=circle_request()["operations"]
    for index,normal in [(0,[0.,0.,1.]),(1,[0.,0.,1.]),(2,[0.,0.,1.]),(3,[0.,0.,1.]),
                         (4,[0.,0.,1.]),(6,[0.,0.,1.]),(13,[0.,0.,1.]),(15,[0.,0.,1.]),
                         (17,[0.,0.,1.]),(18,[0.,0.,1.]),(32,[0.,0.,1.]),(37,[0.,0.,1.]),
                         (3,[0.,0.,-1.]),(3,[1.,2.,3.])]:
        rows.append(dict(op="maelstrom_fit_points_command",id="maelstrom-fit-points-"+str(len(rows)),
                         points=circles[index]["points"],normal=normal,target=5.,degrees=90.,target_kind="lines"))
    for index in (13,32,37):
        rows.append(dict(op="maelstrom_fit_points_command",id="maelstrom-fit-points-"+str(len(rows)),
                         points=circles[index]["points"],normal=[0.,0.,1.],target=5.,degrees=90.,target_kind="points"))
    for op in rows: validate(op)
    return dict(protocol_version=1,iterations=1,operations=rows)


def run(op,host):
    from join_probe import observe_command
    from number_token import number_token
    validate(op)
    Rhino,System=host["Rhino"],host["System"]
    doc=Rhino.RhinoDoc.ActiveDoc;vp=doc.Views.ActiveView.ActiveViewport;saved=vp.GetConstructionPlane()
    if list(doc.Objects):raise ValueError("FitPoints command requires empty owned document")
    lines=op["target_kind"]=="lines"
    targets=([[2.,1.,-2.],[2.,1.,0.],[2.,1.,5.],[2.,1.,12.],[5.,1.,3.],[-4.,3.,2.],[1.,-3.,4.],[0.,0.,0.]]
             if lines else op["points"])
    ends=[[p[0],p[1],p[2]+.25] for p in targets] if lines else []
    target_ids=[];definition_ids=[]
    def circle_record(c):
        return dict(origin=host["_xyz"](c.Center),x=host["_xyz"](c.Plane.XAxis),y=host["_xyz"](c.Plane.YAxis),normal=host["_xyz"](c.Normal),radius=c.Radius)
    def snapshot():
        rows=[]
        for obj in sorted(list(doc.Objects),key=lambda obj:obj.RuntimeSerialNumber):
            row=dict(target=target_ids.index(obj.Id) if obj.Id in target_ids else None,
                             definition=definition_ids.index(obj.Id) if obj.Id in definition_ids else None,
                             selected=bool(obj.IsSelected(False)))
            if isinstance(obj.Geometry,Rhino.Geometry.Point):
                row.update(kind="point",point=host["_xyz"](obj.Geometry.Location))
            elif isinstance(obj.Geometry,Rhino.Geometry.Curve):
                row.update(kind="curve",point=host["_xyz"](obj.Geometry.PointAtStart),end=host["_xyz"](obj.Geometry.PointAtEnd))
            else:raise ValueError("unexpected FitPoints object")
            rows.append(row)
        return rows
    try:
        vp.SetConstructionPlane(Rhino.Geometry.Plane(Rhino.Geometry.Point3d.Origin,Rhino.Geometry.Vector3d(*op["normal"])))
        ok,circle=Rhino.Geometry.Circle.TryFitCircleToPoints([host["_point"](p) for p in op["points"]])
        if not ok or circle.Radius<=0:raise ValueError("FitPoints reference must have positive radius")
        sdk_circle=circle_record(circle)
        morph=Rhino.Geometry.Morphs.MaelstromSpaceMorph(circle.Plane,circle.Radius,op["target"],math.radians(op["degrees"]))
        try:
            sdk_points=[host["_xyz"](morph.MorphPoint(host["_point"](p))) for p in targets]
            sdk_ends=[host["_xyz"](morph.MorphPoint(host["_point"](p))) for p in ends]
        finally:morph.Dispose()
        serial=doc.BeginUndoRecord("Maelstrom FitPoints sources")
        try:
            target_ids=([doc.Objects.AddLine(host["_point"](p),host["_point"](e)) for p,e in zip(targets,ends)]
                        if lines else [doc.Objects.AddPoint(host["_point"](p)) for p in targets])
            definition_ids=[doc.Objects.AddPoint(host["_point"](p)) for p in op["points"]] if lines else []
            if any(i==System.Guid.Empty for i in target_ids+definition_ids):raise ValueError("FitPoints source insertion failed")
            for i in target_ids:doc.Objects.Select(i)
        finally:doc.EndUndoRecord(serial)
        before=snapshot();marker="Viboceros Maelstrom FitPoints "+str(System.Guid.NewGuid());Rhino.RhinoApp.WriteLine(marker)
        picks=" ".join("_SelID "+str(i) for i in definition_ids)
        selection=picks+" _Enter " if lines else ""
        macro="_Maelstrom _FitPoints "+selection+"_Copy=_No _Rigid=_No "+number_token(op["target"])+" "+number_token(op["degrees"])+" _Cancel"
        host["_record_progress"](op["id"]+" "+macro)
        success,after,events=observe_command(Rhino.Commands.Command,"Maelstrom",lambda:Rhino.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        history=Rhino.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        Rhino.RhinoApp.RunScript("_Undo",False);undo=snapshot();Rhino.RhinoApp.RunScript("_Redo",False);redo=snapshot()
        for i in target_ids:doc.Objects.Delete(i,True)
        if not lines:
            definition_ids=[doc.Objects.AddPoint(host["_point"](p)) for p in op["points"]]
        Rhino.RhinoApp.RunScript("_SelNone _Circle _FitPoints _SelAll _Enter _Cancel",False)
        circles=[]
        for obj in list(doc.Objects):
            if isinstance(obj.Geometry,Rhino.Geometry.Curve):
                ok,c=obj.Geometry.TryGetCircle()
                if not ok:raise ValueError("reference Circle is not analytic")
                circles.append(circle_record(c))
        if len(circles)!=1:raise ValueError("FitPoints reference did not create one Circle")
        return dict(before=before,after=after,undo=undo,redo=redo,success=success,events=events,history=history,
                    script_macro=macro,sdk_circle=sdk_circle,sdk_points=sdk_points,sdk_ends=sdk_ends,circle_command=circles[0]),0
    finally:
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        vp.SetConstructionPlane(saved.Plane)
