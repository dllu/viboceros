"""Bounded public ArrayCrv getters in owned empty documents at idle."""
import math

def validate(op):
    required={'op','id','path','representation','spacing','value','orientation','base_point','cplane','reverse_path','postselect','source_count','grouped','subcurve','pick_point','finish','seed_names','edits','input'}
    if not isinstance(op,dict)or set(op)!=required or op['op']!='curve_array':raise ValueError('invalid curve array recipe')
    def scalar(n):return type(n)in(int,float)and not math.isnan(n)and not math.isinf(n)and abs(n)<=1e6
    def point(p):return isinstance(p,list)and len(p)==3 and all(scalar(n)for n in p)
    if op['path']not in('LineX','LineY','LineZ','Arc','Circle','Nurbs','Polyline','PolylineLong','ClosedNurbs','Edge','Folded','Uneven','PlanarPolyline')or op['representation']not in('Native','Nurbs','Polycurve'):raise ValueError('invalid path recipe')
    if op['spacing']not in('Items','Distance')or op['orientation']not in('Freeform','Roadlike','Stairlike','NoRotation'):raise ValueError('invalid curve array mode')
    if op['spacing']=='Items'and(type(op['value'])is not int or not 0<=op['value']<=16)or op['spacing']=='Distance'and(not scalar(op['value'])or not 0<op['value']<=100):raise ValueError('invalid array spacing')
    if any(type(op[k])is not bool for k in('reverse_path','postselect','grouped'))or type(op['source_count'])is not int or not 1<=op['source_count']<=3:raise ValueError('invalid source controls')
    if op['base_point']is not None and not point(op['base_point'])or not isinstance(op['cplane'],list)or len(op['cplane'])!=3 or not all(point(p)for p in op['cplane']):raise ValueError('invalid source frame')
    if op['pick_point']is not None and not point(op['pick_point']):raise ValueError('invalid guide pick')
    if op['path']=='Edge'and(op['reverse_path']or op['representation']!='Native'):raise ValueError('edge recipes retain their authored parent orientation')
    if op['path']=='Edge'and op['pick_point']is None:raise ValueError('edge inputs require a physical pick')
    if op['finish']not in('Accept','CancelGuide','CancelBase','CancelSpacing','CancelAfterValue'):raise ValueError('invalid array finish')
    if op['input']not in('Number','Option')or not isinstance(op['edits'],list)or len(op['edits'])>8:raise ValueError('invalid array option inputs')
    for edit in op['edits']:
        if not isinstance(edit,dict)or len(edit)!=1:raise ValueError('invalid array preview edit')
        key=next(iter(edit));value=edit[key]
        if not(key=='Items'and type(value)is int and 1<=value<=16 or key=='Distance'and scalar(value)and 0<value<=100 or key=='Orientation'and value in('Freeform','Roadlike','Stairlike','NoRotation')):raise ValueError('invalid array option edit')
    r=op['subcurve']
    if r is not None and(not isinstance(r,list)or len(r)!=2 or any(not scalar(n)or not 0<=n<=1 for n in r)or r[0]==r[1]):raise ValueError('invalid temporary range')
    if not isinstance(op['seed_names'],list)or len(op['seed_names'])>8 or any(not isinstance(n,(str,type(u'')))or not 1<=len(n)<=64 for n in op['seed_names']):raise ValueError('invalid group seeds')

def run(op,host,iterations,tolerance):
    validate(op)
    if iterations!=1:raise ValueError('one owned curve array required')
    from join_probe import observe_command
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc;vp=doc.Views.ActiveView.ActiveViewport
    if list(doc.Objects)or doc.UndoRecordingIsActive:raise ValueError('curve array requires an empty owned document at idle')
    old_plane=vp.GetConstructionPlane();old_view=R.DocObjects.ViewportInfo(vp);old_tol=doc.ModelAbsoluteTolerance
    if op['path']in('Polyline','PolylineLong'):
        scale=4. if op['path']=='PolylineLong'else 1.
        path=G.PolylineCurve([G.Point3d(scale*x,scale*y,scale*z)for x,y,z in[(0.,0.,0.),(1.,0.,0.),(1.,1.,0.),(1.,1.,1.)]])
    elif op['path']=='PlanarPolyline':path=G.PolylineCurve([G.Point3d(*p)for p in[(0.,0.,0.),(4.,0.,0.),(4.,4.,0.),(0.,4.,0.)]])
    elif op['path']=='Uneven':path=G.PolylineCurve([G.Point3d(*p)for p in[(0.,0.,0.),(100.,0.,0.),(101.,0.,0.),(102.,0.,0.)]])
    elif op['path']=='Folded':path=G.PolylineCurve([G.Point3d(*p)for p in[(0.,0.,0.),(10.,0.,0.),(10.,10.,0.),(0.,1.,0.)]])
    elif op['path']=='Nurbs':path=G.Curve.CreateControlPointCurve([G.Point3d(*p)for p in[(0.,0.,0.),(2.,0.,3.),(4.,3.,-1.),(7.,5.,4.),(10.,8.,6.)]],3)
    elif op['path']=='ClosedNurbs':path=G.Curve.CreateInterpolatedCurve([G.Point3d(*p)for p in[(5.,0.,0.),(1.,5.,2.),(-5.,1.,-1.),(-1.,-5.,3.),(5.,0.,0.)]],3,G.CurveKnotStyle.ChordPeriodic)
    elif op['path']=='Arc':path=G.ArcCurve(G.Arc(G.Point3d(5.,0.,0.),G.Point3d(0.,3.,4.),G.Point3d(-5.,0.,0.)))
    elif op['path']=='Circle':path=G.Circle(G.Plane(G.Point3d.Origin,G.Vector3d(0.,.6,.8)),5.).ToNurbsCurve()
    else:path=G.LineCurve(G.Point3d.Origin,G.Point3d(10.,0.,0.)if op['path']=='LineX'else G.Point3d(0.,10.,0.)if op['path']=='LineY'else G.Point3d(0.,0.,10.))
    parent=None
    if op['path']=='Edge':
        parent=G.NurbsSurface.CreateFromCorners(G.Point3d(0.,-2.,0.),G.Point3d(12.,-2.,0.),G.Point3d(12.,6.,0.),G.Point3d(0.,6.,0.)).ToBrep();path.Dispose();path=parent.Edges[0].DuplicateCurve()
    elif op['representation']=='Nurbs':new=path.ToNurbsCurve();path.Dispose();path=new
    elif op['representation']=='Polycurve':
        segments=path.DuplicateSegments();new=G.PolyCurve()
        for segment in segments:new.Append(segment);segment.Dispose()
        path.Dispose();path=new
    if op['reverse_path']:path.Reverse()
    ids=[];rail=None;used=host.setdefault('_curve_array_used_names',set())
    def snapshot():
        used.update(g.Name for g in doc.Groups if not g.IsDeleted and g.Name)
        rows=[]
        for o in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
            g=o.Geometry
            if isinstance(g,G.PointCloud):geometry=dict(kind='PointCloud',points=[host['_xyz'](p.Location)for p in g])
            elif isinstance(g,G.Brep):geometry=dict(kind='Brep',surface=host['_nurbs_surface_definition'](g.Surfaces[0]),reversed=False,edge=host['_nurbs_curve_definition'](g.Edges[0]))
            elif isinstance(g,G.Curve):geometry=dict(kind=g.GetType().Name,curve=host['_nurbs_curve_definition'](g))
            else:raise ValueError('unsupported curve array object')
            rows.append(dict(role='source'if o.Id in ids else'rail'if o.Id==rail else'output',source=ids.index(o.Id)if o.Id in ids else None,geometry=geometry,name=o.Attributes.Name,selected=bool(o.IsSelected(False)),groups=list(o.Attributes.GetGroupList()or[])))
        return dict(objects=rows,groups=[dict(index=g.Index,name=g.Name)for g in doc.Groups if not g.IsDeleted],used_names=sorted(used))
    try:
        doc.ModelAbsoluteTolerance=1e-6;origin,x,y=op['cplane'];vp.SetConstructionPlane(G.Plane(G.Point3d(*origin),G.Vector3d(*x),G.Vector3d(*y)))
        record=doc.BeginUndoRecord('Owned curve array scene')
        try:
            for name in op['seed_names']:
                i=doc.Groups.Add(name)
                if i<0:raise ValueError('failed owned group seed')
                used.add(name);doc.Groups.Delete(i)
            for i in range(op['source_count']):
                a=R.DocObjects.ObjectAttributes();a.Name='Curve array source '+str(i);cloud=G.PointCloud([G.Point3d(p[0]+3.*i,p[1]+2.*i,p[2])for p in[[0.,0.,0.],[1.,0.,0.],[0.,1.,0.],[0.,0.,1.],[2.,1.,.5]]]);ids.append(doc.Objects.AddPointCloud(cloud,a));cloud.Dispose()
            if op['grouped']:doc.Groups.Add('Curve array group',ids)
            a=R.DocObjects.ObjectAttributes();a.Name='Curve array rail';rail=doc.Objects.AddBrep(parent,a)if parent is not None else doc.Objects.AddCurve(path,a)
        finally:doc.EndUndoRecord(record)
        if not op['postselect']:
            for id in ids:doc.Objects.Select(id)
        doc.ClearUndoRecords(True);before=snapshot();crc=doc.Objects.FindId(rail).Geometry.DataCRC(0)
        actions=[];prompts=[]
        def action(value,prefix):actions.append(value);prompts.append([prefix]if isinstance(prefix,(str,type(u'')))else prefix)
        if op['postselect']:
            for i in range(len(ids)):action(dict(curve=1+i),'Select objects to array')
            action('Enter','Select objects to array')
        if op['finish']=='CancelGuide':action('Cancel','Select path curve')
        else:
            if op['base_point']is not None:
                action('Basepoint','Select path curve');action('Cancel'if op['finish']=='CancelBase'else op['base_point'],'Base point for objects to array')
            if op['finish']!='CancelBase':
                if op['subcurve']is not None:action('SubCrv','Select path curve')
                pick=dict(curve=0)
                if op['pick_point']is not None:pick['pick']=op['pick_point']
                if op['path']=='Edge':pick['edge']=0
                action(pick,'Select curve to temporarily shorten'if op['subcurve']is not None else'Select path curve')
                if op['subcurve']is not None:
                    for i,fraction in enumerate(op['subcurve']):
                        action(host['_xyz'](path.PointAt(path.Domain.ParameterAt(fraction))),'Start of curve'if i==0 else'Current length')
                ready='Array by '
                action('Orientation='+op['orientation'],ready)
                if op['finish']=='CancelSpacing':action('Cancel',ready)
                else:
                    if op['spacing']=='Distance'or op['input']=='Option':
                        action(op['spacing'],ready)
                        action(op['value'],['Distance between items','Number of items'])
                    else:action(op['value'],ready)
                    for edit in op['edits']:
                        key=next(iter(edit))
                        if key=='Orientation':action(key+'='+str(edit[key]),ready)
                        else:action(key,ready);action(edit[key],['Number of items','Distance between items'])
                    if op['finish']=='CancelAfterValue':action('Cancel',[ready,'Number of items','Distance between items'])
                    elif op['spacing']=='Distance'or op['input']=='Option':action('Enter',ready)
        marker='Owned curve array '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker);traces=[]
        def execute():
            from conic_getter_input import drive
            result,trace=drive(dict(op,actions=actions,action_prompts=prompts),host,[rail]+ids,command='ArrayCrv');traces.append(trace);return result
        success,after,events=observe_command(R.Commands.Command,'ArrayCrv',execute,snapshot,lambda:[],True)
        after_script=snapshot();history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip();R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot();undo_history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()[len(history):]
        frame_audit=[]
        if op['path']in('Polyline','PolylineLong')and op['orientation']=='Freeform':
            for fractions in[[0.,1.],[0.,1./3.,2./3.,1.],[0.,.3,.6,1.],[0.,1./3.,1.],[0.,2./3.,1.]]:
                params=[path.Domain.ParameterAt(f)for f in fractions];frames=path.GetPerpendicularFrames(params)
                frame_audit.append(dict(parameters=params,frames=None if frames is None else[dict(origin=host['_xyz'](f.Origin),x=host['_xyz'](f.XAxis),y=host['_xyz'](f.YAxis),z=host['_xyz'](f.ZAxis))for f in frames]))
        return dict(frame_audit=frame_audit,success=success,actions=actions,action_prompts=prompts,input_trace=traces[0],before=before,after=after,after_script=after_script,undo=undo,redo=redo,events=events,history=history,undo_history=undo_history,guide_unchanged=doc.Objects.FindId(rail).Geometry.DataCRC(0)==crc),0
    finally:
        for o in list(doc.Objects):doc.Objects.Delete(o.Id,True)
        for group in list(doc.Groups):
            if not group.IsDeleted:doc.Groups.Delete(group.Index)
        path.Dispose()
        if parent is not None:parent.Dispose()
        vp.SetViewProjection(old_view,False);old_view.Dispose();vp.SetConstructionPlane(old_plane);doc.ModelAbsoluteTolerance=old_tol
