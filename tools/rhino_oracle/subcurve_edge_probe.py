"""Owned public SubCrv input from selected B-rep edges, preserving the parent."""
import re
CASES=('point_default','point_copy_no','point_copy_yes','point_backward','mark_ends','midpoint','surface_point','curved_point','curved_backward','curved_mark','curved_midpoint')
def request():return dict(protocol_version=1,iterations=1,operations=[dict(op='subcurve_edge',id='edge_subcurve_'+c,case=c) for c in CASES])
def validate_request(q):
    if not isinstance(q,dict) or type(q.get('protocol_version')) is not int or q['protocol_version']!=1 or type(q.get('iterations',1)) is not int or q.get('iterations',1)!=1 or not isinstance(q.get('operations'),list) or not 1<=len(q['operations'])<=32:raise ValueError('edge SubCrv requires bounded protocol 1 recipes')
    seen=set()
    for op in q['operations']:
        if not isinstance(op,dict) or set(op)!={'op','id','case'} or op['op']!='subcurve_edge' or not isinstance(op['id'],str) or re.match(r'^[A-Za-z0-9_-]{1,80}\Z',op['id']) is None or op['id'] in seen or op.get('case') not in CASES:raise ValueError('invalid edge SubCrv recipe')
        seen.add(op['id'])
def run(op,host):
    validate_request(dict(protocol_version=1,operations=[op]))
    Rhino,System=host['Rhino'],host['System'];G=Rhino.Geometry;doc=Rhino.RhinoDoc.ActiveDoc
    if Rhino.Commands.Command.InCommand():raise ValueError('edge SubCrv requires idle execution')
    if list(doc.Objects):raise ValueError('edge SubCrv requires empty owned document')
    from join_probe import observe_command
    saved_layer=doc.Layers.CurrentLayerIndex;saved_tolerance=doc.ModelAbsoluteTolerance
    baseline_groups=set(i for i in range(doc.Groups.Count) if not doc.Groups.IsDeleted(i))
    layers=[];owned=[];source_id=None
    doc.ModelAbsoluteTolerance=1e-6
    def record_geometry(g):
        if isinstance(g,G.Brep):return dict(kind='brep',definition=host['_interchange_brep_record'](g,include_samples=False))
        if isinstance(g,G.Point):return dict(kind='point',point=host['_xyz'](g.Location))
        n=g.ToNurbsCurve()
        try:return dict(kind='curve',definition=host['_nurbs_curve_definition'](n),samples=[host['_xyz'](g.PointAt(g.Domain.ParameterAt(i/32.))) for i in range(33)])
        finally:n.Dispose()
    def snapshot():
        rows=[]
        for obj in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
            a=obj.Attributes;row=dict(source=0 if obj.Id==source_id else None,selected=bool(obj.IsSelected(False)),name=a.Name,layer=doc.Layers[a.LayerIndex].Name,user_text=a.GetUserString('source'),groups=list(a.GetGroupList() or []),components=[[str(c.ComponentIndexType),int(c.Index)] for c in (obj.GetSelectedSubObjects() or [])])
            row.update(record_geometry(obj.Geometry));rows.append(row)
        return rows
    try:
        for suffix in ('input','output'):
            layer=Rhino.DocObjects.Layer();layer.Name=op['id']+'_'+suffix;layers.append(doc.Layers.Add(layer));layer.Dispose()
        case=op['case']
        box=G.Brep.CreateFromBox(G.BoundingBox(G.Point3d(0.,0.,0.),G.Point3d(4.,6.,2.)));owned.append(box)
        if case=='surface_point':
            surface=G.NurbsSurface.CreateFromCorners(G.Point3d(0.,0.,0.),G.Point3d(4.,0.,0.),G.Point3d(4.,6.,0.),G.Point3d(0.,6.,0.));owned.append(surface);box=surface.ToBrep();owned.append(box)
        elif case.startswith('curved_'):
            box=G.Brep.CreateFromCylinder(G.Cylinder(G.Circle(G.Plane.WorldXY,2.),3.),True,True);owned.append(box)
        if box is None or not box.IsValid:raise ValueError('invalid edge SubCrv parent')
        matches=[e for e in box.Edges if (e.IsClosed and abs(e.PointAtStart.Z)<1e-9) ] if case.startswith('curved_') else [e for e in box.Edges if {tuple(host['_xyz'](e.PointAtStart)),tuple(host['_xyz'](e.PointAtEnd))}=={(0.,0.,0.),(4.,0.,0.)}]
        if len(matches)!=1:raise ValueError('expected one owned horizontal edge')
        edge=matches[0];edge_index=int(edge.EdgeIndex);n=edge.ToNurbsCurve();owned.append(n)
        definition=host['_nurbs_curve_definition'](n)
        group=doc.Groups.Add('SubCrv_edge_parent_'+op['id'])
        a=doc.CreateDefaultAttributes();a.LayerIndex=layers[0];a.Name='parent';a.SetUserString('source','original');a.AddToGroup(group)
        source_id=doc.Objects.AddBrep(box,a);a.Dispose();doc.Layers.SetCurrentLayerIndex(layers[1],True)
        obj=doc.Objects.FindId(source_id)
        if not obj.SelectSubObject(G.ComponentIndex(G.ComponentIndexType.BrepEdge,edge_index),True,True,False):raise ValueError('owned edge preselection failed')
        case=op['case'];fractions=(.75,.25) if case in ('point_backward','curved_backward') else ((.5,.75) if case in ('midpoint','curved_midpoint') else (.25,.75))
        parameters=[edge.Domain.ParameterAt(f) for f in fractions];points=[host['_xyz'](edge.PointAt(t)) for t in parameters]
        options='_Mode=_'+('MarkEnds' if case in ('mark_ends','curved_mark') else 'Shorten')+' _FromMidpoint=_'+('Yes' if case in ('midpoint','curved_midpoint') else 'No')
        if case=='point_copy_no':options+=' _Copy=_No'
        elif case=='point_copy_yes':options+=' _Copy=_Yes'
        macro='_SubCrv '+options+' '+' '.join(','.join(repr(x) for x in p) for p in points)+' _Enter'
        doc.ClearUndoRecords(True);before=snapshot();marker='Viboceros edge SubCrv '+str(System.Guid.NewGuid());Rhino.RhinoApp.WriteLine(marker)
        success,after,events=observe_command(Rhino.Commands.Command,'SubCrv',lambda:Rhino.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        active=bool(Rhino.Commands.Command.InCommand())
        if active:Rhino.RhinoApp.RunScript('!',False)
        after_script=snapshot();history=Rhino.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1]
        Rhino.RhinoApp.RunScript('_Undo',False);undo=snapshot();Rhino.RhinoApp.RunScript('_Redo',False);redo=snapshot()
        return dict(case=case,edge_index=edge_index,edge_definition=definition,parameters=parameters,points=points,macro=macro,before=before,after=after,after_script=after_script,success=success,command_active=active,history=history,events=events,undo=undo,redo=redo),0
    finally:
        if Rhino.Commands.Command.InCommand():Rhino.RhinoApp.RunScript('!',False)
        doc.Objects.UnselectAll()
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        doc.Layers.SetCurrentLayerIndex(saved_layer,True)
        for i in range(doc.Groups.Count):
            if i not in baseline_groups and not doc.Groups.IsDeleted(i):doc.Groups.Delete(i)
        for i in reversed(layers):doc.Layers.Delete(i,True)
        doc.ModelAbsoluteTolerance=saved_tolerance
        for g in reversed(owned):
            if g is not None:g.Dispose()
