"""Bounded public ArraySrf commands in an isolated, owned empty document."""
import math

def validate(op):
    required={'op','id','scene','counts','mode','base_point','up_point','cplane','reverse_face','postselect','source_count','grouped','preview','edits','finish','seed_names','domains','swap_uv'}
    if not isinstance(op,dict)or set(op)!=required or op['op']!='array_surface':raise ValueError('invalid surface array recipe')
    def scalar(n):return type(n)in(int,float)and not math.isnan(n)and not math.isinf(n)and abs(n)<=1e6
    def point(p):return isinstance(p,list)and len(p)==3 and all(scalar(n)for n in p)
    def count(n):return type(n)is int and 1<=n<=16
    if op['scene']not in('Plane','Tilted','Warp','DoubleWarp','Cylinder','Sphere','TrimmedPlane'):raise ValueError('invalid array surface')
    if any(type(op[k])is not bool for k in('reverse_face','postselect','grouped','preview','swap_uv'))or type(op['source_count'])is not int or not 1<=op['source_count']<=3:raise ValueError('invalid surface array controls')
    if not isinstance(op['counts'],list)or len(op['counts'])!=2 or not all(count(n)for n in op['counts'])or op['mode']not in('UV','Isocurve'):raise ValueError('invalid counts or mode')
    if not point(op['base_point'])or op['up_point']is not None and not point(op['up_point'])or not isinstance(op['cplane'],list)or len(op['cplane'])!=3 or not all(point(p)for p in op['cplane']):raise ValueError('invalid source frame')
    if op['finish']not in('Enter','Cancel','CancelBase','CancelUp','CancelSurface','CancelU','CancelV'):raise ValueError('invalid finish')
    if not isinstance(op['edits'],list)or len(op['edits'])>8:raise ValueError('invalid preview edits')
    for edit in op['edits']:
        if not isinstance(edit,dict)or len(edit)!=1:raise ValueError('invalid preview edit')
        name=next(iter(edit));value=edit[name]
        if not(name in('UNumber','VNumber')and count(value)or name=='Mode'and value in('UV','Isocurve')):raise ValueError('invalid preview option')
    if not op['preview']and op['edits']:raise ValueError('edits require preview')
    if not isinstance(op['seed_names'],list)or len(op['seed_names'])>8 or any(not isinstance(n,(str,type(u'')))or not 1<=len(n)<=64 for n in op['seed_names']):raise ValueError('invalid group seeds')
    d=op['domains']
    if not isinstance(d,list)or len(d)!=2 or any(not isinstance(v,list)or len(v)!=2 or not all(scalar(n)for n in v)or v[0]>=v[1]for v in d):raise ValueError('invalid surface domains')

def run(op,host,iterations,tolerance):
    validate(op)
    if iterations!=1:raise ValueError('one owned surface array required')
    from join_probe import observe_command
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc;vp=doc.Views.ActiveView.ActiveViewport
    if list(doc.Objects)or doc.UndoRecordingIsActive:raise ValueError('surface array requires an empty owned document at idle')
    old_plane=vp.GetConstructionPlane();old_tolerance=doc.ModelAbsoluteTolerance
    plane=G.Plane(G.Point3d(0.,0.,5.),G.Vector3d.ZAxis)
    if op['scene']=='Tilted':plane=G.Plane(G.Point3d(3.,-4.,5.),G.Vector3d(.6,.8,0.),G.Vector3d(-.48,.36,.8))
    if op['scene']=='Cylinder':surface=G.Cylinder(G.Circle(G.Plane.WorldXY,10.),20.).ToNurbsSurface()
    elif op['scene']=='Sphere':surface=G.Sphere(G.Point3d.Origin,10.).ToNurbsSurface()
    elif op['scene']in('Warp','DoubleWarp'):
        surface=G.NurbsSurface.Create(3,False,3,3,3,3)
        for v in range(3):
            for u in range(3):surface.Points.SetControlPoint(u,v,G.ControlPoint(G.Point3d(6.*u,6.*v,5.+1.5*u*v+.5*u*u+(1.25*v*v if op['scene']=='DoubleWarp'else 0.))))
        for knots in(surface.KnotsU,surface.KnotsV):
            for i,n in enumerate([0.,0.,1.,1.]):knots[i]=n
    else:surface=G.PlaneSurface(plane,G.Interval(-10.,10.),G.Interval(-5.,15.)).ToNurbsSurface()
    if op['swap_uv']:surface.Transpose()
    for axis,domain in enumerate(op['domains']):surface.SetDomain(axis,G.Interval(*domain))
    target=surface.ToBrep()
    trim_range=None
    if op['scene']=='TrimmedPlane':
        u,v=surface.Domain(0),surface.Domain(1)
        trim_range=[[u.ParameterAt(.2),u.ParameterAt(.8)],[v.ParameterAt(.3),v.ParameterAt(.7)]]
        a,b=trim_range[0];c,d=trim_range[1];uvs=[(a,c),(b,c),(b,d),(a,d),(a,c)]
        spatial=G.PolylineCurve([surface.PointAt(u,v)for u,v in uvs]);parameter=G.PolylineCurve([G.Point3d(u,v,0.)for u,v in uvs])
        definition=host['_nurbs_surface_definition'](surface);definition.update(degree_u=surface.Degree(0),degree_v=surface.Degree(1),control_point_count_u=surface.Points.CountU,control_point_count_v=surface.Points.CountV)
        target.Dispose();target=host['_trimmed_brep_from_definition'](dict(surface=definition,boundaries=[dict(curve=host['_nurbs_curve_definition'](spatial),parameter_curve=host['_nurbs_curve_definition'](parameter))],interior_uv=[(a+b)*.5,(c+d)*.5]),dict(absolute=1e-6,relative=1e-12,angular=1e-6))
        spatial.Dispose();parameter.Dispose()
    if op['reverse_face']:target.Flip()
    sources=[];target_id=None;used=host.setdefault('_array_surface_used_names',set())
    def snapshot():
        used.update(g.Name for g in doc.Groups if not g.IsDeleted and g.Name)
        rows=[]
        for o in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
            g=o.Geometry
            if isinstance(g,G.PointCloud):geometry=dict(kind='PointCloud',points=[host['_xyz'](p.Location)for p in g])
            elif isinstance(g,G.Brep):geometry=dict(kind='Brep',surface=host['_nurbs_surface_definition'](g.Surfaces[0]),face_domains=[[g.Faces[0].Domain(i).T0,g.Faces[0].Domain(i).T1]for i in range(2)],trim_range=trim_range,reversed=bool(g.Faces[0].OrientationIsReversed))
            else:raise ValueError('unsupported surface array object')
            rows.append(dict(role='source'if o.Id in sources else'target'if o.Id==target_id else'output',source=sources.index(o.Id)if o.Id in sources else None,name=o.Attributes.Name,selected=bool(o.IsSelected(False)),groups=list(o.Attributes.GetGroupList()or[]),geometry=geometry))
        return dict(objects=rows,groups=[dict(index=g.Index,name=g.Name)for g in doc.Groups if not g.IsDeleted],used_names=sorted(used))
    try:
        doc.ModelAbsoluteTolerance=1e-6;origin,x,y=op['cplane'];vp.SetConstructionPlane(G.Plane(G.Point3d(*origin),G.Vector3d(*x),G.Vector3d(*y)))
        record=doc.BeginUndoRecord('Owned surface array scene')
        try:
            for name in op['seed_names']:
                index=doc.Groups.Add(name)
                if index<0:raise ValueError('failed group seed')
                used.add(name);doc.Groups.Delete(index)
            for i in range(op['source_count']):
                attributes=R.DocObjects.ObjectAttributes();attributes.Name='Surface array source '+str(i)
                cloud=G.PointCloud([G.Point3d(p[0]+3.*i,p[1]+2.*i,p[2])for p in[[0.,0.,0.],[1.,0.,0.],[0.,1.,0.],[0.,0.,1.],[2.,1.,.5]]]);sources.append(doc.Objects.AddPointCloud(cloud,attributes));cloud.Dispose()
            if op['grouped']:doc.Groups.Add('Surface array group',sources)
            attributes=R.DocObjects.ObjectAttributes();attributes.Name='Surface array target';target_id=doc.Objects.AddBrep(target,attributes)
        finally:doc.EndUndoRecord(record)
        if not op['postselect']:
            for id in sources:doc.Objects.Select(id)
        doc.ClearUndoRecords(True);before=snapshot();crc=doc.Objects.FindId(target_id).Geometry.DataCRC(0)
        actions=[];prompts=[]
        def action(v,prefix):actions.append(v);prompts.append([prefix])
        if op['postselect']:
            for i in range(len(sources)):action(dict(curve=1+i),'Select objects to array')
            action('Enter','Select objects to array')
        action('Mode='+op['mode'],'Base point for objects to array');action('Preview='+('Yes'if op['preview']else'No'),'Base point for objects to array')
        if op['finish']=='CancelBase':action('Cancel','Base point for objects to array')
        else:
            action(op['base_point'],'Base point for objects to array')
            if op['finish']=='CancelUp':action('Cancel','Reference normal for objects to array')
            else:
                action('Enter'if op['up_point']is None else op['up_point'],'Reference normal for objects to array')
                if op['finish']=='CancelSurface':action('Cancel','Select target surface for array')
                else:
                    action(dict(curve=0),'Select target surface for array')
                    if op['finish']=='CancelU':action('Cancel','Number of elements in surface u direction')
                    else:
                        action(op['counts'][0],'Number of elements in surface u direction')
                        if op['finish']=='CancelV':action('Cancel','Number of elements in surface v direction')
                        else:
                            action(op['counts'][1],'Number of elements in surface v direction')
                            if op['preview']:
                                for edit in op['edits']:
                                    name=next(iter(edit));action(name+'='+str(edit[name]),'Press Enter to accept')
                                action(op['finish'],'Press Enter to accept')
        traces=[];marker='Owned surface array '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        def execute():
            from conic_getter_input import drive
            result,trace=drive(dict(op,actions=actions,action_prompts=prompts),host,[target_id]+sources,command='ArraySrf');traces.append(trace);return result
        success,after,events=observe_command(R.Commands.Command,'ArraySrf',execute,snapshot,lambda:[],True)
        after_script=snapshot();history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip();R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot();undo_history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()[len(history):]
        return dict(success=success,actions=actions,action_prompts=prompts,input_trace=traces[0],before=before,after=after,after_script=after_script,undo=undo,redo=redo,events=events,history=history,undo_history=undo_history,target_unchanged=doc.Objects.FindId(target_id).Geometry.DataCRC(0)==crc),0
    finally:
        for o in list(doc.Objects):doc.Objects.Delete(o.Id,True)
        for g in list(doc.Groups):
            if not g.IsDeleted:doc.Groups.Delete(g.Index)
        surface.Dispose();target.Dispose();vp.SetConstructionPlane(old_plane);doc.ModelAbsoluteTolerance=old_tolerance
