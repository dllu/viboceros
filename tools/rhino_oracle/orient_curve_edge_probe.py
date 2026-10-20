"""Owned public OrientCrvToEdge getters and immutable geometry observations."""
import math,re
STR=(str,type(u''))

def validate(op):
    required={'op','id','points','corners','reversed','kind','grouped','cplane','actions','action_prompts'}
    if not isinstance(op,dict)or set(op)-{'angle_tolerance'}!=required or op['op']!='orient_curve_edge':raise ValueError('invalid curve to edge recipe')
    if 'angle_tolerance'in op and(type(op['angle_tolerance'])not in(int,float)or not 0<op['angle_tolerance']<=1):raise ValueError('invalid owned angle tolerance')
    if not isinstance(op['id'],STR)or re.match(r'\A[A-Za-z0-9_][A-Za-z0-9_-]{0,79}\Z',op['id'])is None:raise ValueError('invalid case identifier')
    def point(p):return isinstance(p,list)and len(p)==3 and all(type(v)in(int,float)and not math.isnan(v)and not math.isinf(v)and abs(v)<=1000 for v in p)
    if type(op['grouped'])is not bool:raise ValueError('invalid source group')
    if op['kind']not in('Polyline','Nurbs','Line'):raise ValueError('invalid source type')
    if not isinstance(op['points'],list)or not 2<=len(op['points'])<=16 or not all(point(p)for p in op['points']):raise ValueError('invalid source curve')
    if not isinstance(op['corners'],list)or len(op['corners'])!=4 or not all(point(p)for p in op['corners'])or type(op['reversed'])is not bool:raise ValueError('invalid target surface')
    if not isinstance(op['cplane'],list)or len(op['cplane'])!=3 or not all(point(p)for p in op['cplane']):raise ValueError('invalid construction plane')
    if not isinstance(op['actions'],list)or not 1<=len(op['actions'])<=24:raise ValueError('invalid getter sequence')
    for a in op['actions']:
        if isinstance(a,STR)and a in('Copy=Yes','Copy=No','FlipSurface','ReverseCurve','FlipSurface=Yes','FlipSurface=No','ReverseCurve=Yes','ReverseCurve=No','Enter','Cancel'):continue
        if point(a):continue
        if isinstance(a,dict)and set(a)=={'click'}and point(a['click']):continue
        if isinstance(a,dict)and set(a)in({'curve'},{'curve','pick'},{'curve','pick','edge'})and type(a['curve'])is int and a['curve']in(0,1)and('pick'not in a or point(a['pick']))and('edge'not in a or type(a['edge'])is int and 0<=a['edge']<4):continue
        raise ValueError('invalid getter action')
    gates=op['action_prompts']
    if not isinstance(gates,list)or len(gates)!=len(op['actions'])or any(not isinstance(g,list)or not 1<=len(g)<=4 or any(not isinstance(s,STR)or s not in ('Select curve to orient','Select target surface edge','Pick target edge point') for s in g)for g in gates):raise ValueError('invalid prompt gates')

def run(op,host,iterations,tolerance):
    validate(op)
    from join_probe import observe_command
    from conic_getter_input import drive
    R=host['Rhino'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc;vp=doc.Views.ActiveView.ActiveViewport
    if iterations!=1 or list(doc.Objects)or doc.UndoRecordingIsActive:raise ValueError('curve to edge requires an empty owned document at idle')
    old_plane=vp.GetConstructionPlane();old_view=R.DocObjects.ViewportInfo(vp);old_tol=doc.ModelAbsoluteTolerance;old_angle=doc.ModelAngleToleranceRadians
    source=G.PolylineCurve([G.Point3d(*p)for p in op['points']])if op['kind']=='Polyline'else G.LineCurve(G.Point3d(*op['points'][0]),G.Point3d(*op['points'][-1]))if op['kind']=='Line'else G.Curve.CreateControlPointCurve([G.Point3d(*p)for p in op['points']],min(3,len(op['points'])-1));surface=G.NurbsSurface.CreateFromCorners(*[G.Point3d(*p)for p in op['corners']]).ToBrep()
    if op['reversed']:surface.Flip()
    ids=[]
    def snapshot():
        rows=[]
        for o in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
            g=o.Geometry
            geometry=dict(kind='Brep',crc=int(g.DataCRC(0)))if isinstance(g,G.Brep)else dict(kind=g.GetType().Name,curve=host['_nurbs_curve_definition'](g))
            rows.append(dict(role='source'if o.Id==ids[0]else'target'if o.Id==ids[1]else'output',geometry=geometry,selected=bool(o.IsSelected(False)),name=o.Attributes.Name,groups=[doc.Groups[i].Name for i in (o.Attributes.GetGroupList()or[])]))
        return rows
    def groups():return [dict(index=g.Index,name=g.Name)for g in doc.Groups if not g.IsDeleted]
    try:
        doc.ModelAbsoluteTolerance=1e-6
        if 'angle_tolerance'in op:doc.ModelAngleToleranceRadians=op['angle_tolerance']
        o,x,y=op['cplane'];vp.SetConstructionPlane(G.Plane(G.Point3d(*o),G.Vector3d(*x),G.Vector3d(*y)))
        record=doc.BeginUndoRecord('Owned curve to edge scene')
        try:
            for name,g in[('Source',source),('Target',surface)]:
                a=R.DocObjects.ObjectAttributes();a.Name=name;ids.append(doc.Objects.AddCurve(g,a)if name=='Source'else doc.Objects.AddBrep(g,a))
            if op['grouped']:doc.Groups.Add('Curve edge source group',[ids[0]])
        finally:doc.EndUndoRecord(record)
        doc.ClearUndoRecords(True);before=snapshot();groups_before=groups();marker='Owned curve edge '+str(host['System'].Guid.NewGuid());R.RhinoApp.WriteLine(marker);traces=[]
        def execute():
            result,trace=drive(op,host,ids,'OrientCrvToEdge');traces.append(trace);return result
        success,after,events=observe_command(R.Commands.Command,'OrientCrvToEdge',execute,snapshot,lambda:[],True)
        after_script=snapshot();groups_after=groups();history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        R.RhinoApp.RunScript('_Undo',False);undo=snapshot();groups_undo=groups();R.RhinoApp.RunScript('_Redo',False);redo=snapshot();groups_redo=groups()
        frame_audit=[]
        for t in [source.Domain.T0,source.Domain.T1]:
            for method in ['FrameAt','PerpendicularFrameAt']:
                ok,f=getattr(source,method)(t);frame_audit.append(dict(parameter=float(t),method=method,ok=bool(ok),origin=host['_xyz'](f.Origin),x=host['_xyz'](f.XAxis),y=host['_xyz'](f.YAxis),z=host['_xyz'](f.ZAxis)))
        active_plane=vp.GetConstructionPlane().Plane
        return dict(actual_tolerance=dict(absolute=doc.ModelAbsoluteTolerance,angular=doc.ModelAngleToleranceRadians),groups=dict(before=groups_before,after_script=groups_after,undo=groups_undo,redo=groups_redo),frame_audit=frame_audit,actual_cplane=dict(x=host['_xyz'](active_plane.XAxis),y=host['_xyz'](active_plane.YAxis),z=host['_xyz'](active_plane.ZAxis)),target_definition=host['_nurbs_surface_definition'](surface.Surfaces[0]),target_reversed=bool(surface.Faces[0].OrientationIsReversed),edges=[host['_nurbs_curve_definition'](e)for e in surface.Edges],normal=host['_xyz'](surface.Faces[0].NormalAt(surface.Faces[0].Domain(0).Mid,surface.Faces[0].Domain(1).Mid)),success=success,input_trace=traces[0],before=before,after=after,after_script=after_script,undo=undo,redo=redo,events=events,history=history),0
    finally:
        for o in list(doc.Objects):doc.Objects.Delete(o.Id,True)
        for group in list(doc.Groups):
            if not group.IsDeleted:doc.Groups.Delete(group.Index)
        source.Dispose();surface.Dispose();vp.SetViewProjection(old_view,False);old_view.Dispose();vp.SetConstructionPlane(old_plane);doc.ModelAbsoluteTolerance=old_tol;doc.ModelAngleToleranceRadians=old_angle
