"""Bounded public ArrayCrvOnSrf commands at idle in owned empty documents."""
import math

def validate(op):
    required={'op','id','scene','path','mode','value','base_point','cplane','reverse_path','reverse_face','postselect','source_count','grouped','finish','subcurve','seed_names','pick_point'}
    if not isinstance(op,dict)or set(op)!=required or op['op']!='array_curve_surface':raise ValueError('invalid surface curve array recipe')
    if op['scene']not in('Plane','Tilted','Cylinder','Sphere','Warp','OffSurface')or op['path']not in('LineX','LineY','Circle','IsoU','IsoV','Nurbs','Polyline'):raise ValueError('invalid surface curve array guides')
    if any(type(op[k])is not bool for k in('reverse_path','reverse_face','postselect','grouped'))or type(op['source_count'])is not int or not 1<=op['source_count']<=3 or op['finish']not in('Cancel','CancelReference','EnterCancel'):raise ValueError('invalid surface curve array controls')
    def scalar(n):return type(n)in(int,float)and not math.isnan(n)and not math.isinf(n)and abs(n)<=1e6
    def point(p):return isinstance(p,list)and len(p)==3 and all(scalar(n)for n in p)
    if not point(op['base_point'])or not isinstance(op['cplane'],list)or len(op['cplane'])!=3 or not all(point(p)for p in op['cplane']):raise ValueError('invalid frame or base point')
    def value_valid(mode,value):
        if mode=='Divide':return value is None or type(value)is int and 1<=value<=16
        if mode=='Multiple':return isinstance(value,list)and len(value)==2 and(value[0]is None or type(value[0])is int and 1<=value[0]<=16)and(value[1]is None or scalar(value[1])and abs(value[1])<=100)
        if mode=='Positions':return isinstance(value,list)and 1<=len(value)<=4 and all(point(p)for p in value)
        if mode=='Distances':return isinstance(value,list)and 1<=len(value)<=4 and all(scalar(v)for v in value)
        return False
    mode,value=op['mode'],op['value']
    if mode=='Sequence':
        modes={'divide':'Divide','multiple':'Multiple','point':'Positions','distance':'Distances'}
        if not isinstance(value,list)or not 1<=len(value)<=8:raise ValueError('invalid array sequence')
        for step in value:
            if not isinstance(step,dict)or len(step)!=1:raise ValueError('invalid array sequence step')
            key=next(iter(step));v=step[key]
            if key not in modes or not value_valid(modes[key],[v]if key in('point','distance')else v):raise ValueError('invalid array sequence value')
    elif not value_valid(mode,value):raise ValueError('invalid array placements')
    interval=op['subcurve']
    if interval is not None and(not isinstance(interval,list)or len(interval)!=2 or not all(scalar(v)and 0<=v<=1 for v in interval)or interval[0]==interval[1]):raise ValueError('invalid temporary interval')
    if op['pick_point']is not None and not point(op['pick_point']):raise ValueError('invalid path pick')
    if not isinstance(op['seed_names'],list)or len(op['seed_names'])>8 or any(not isinstance(n,(str,type(u'')))or not 1<=len(n)<=64 for n in op['seed_names']):raise ValueError('invalid prior group names')

def run(op,host,iterations,tolerance):
    validate(op)
    if iterations!=1:raise ValueError('one owned surface curve array required')
    from join_probe import observe_command
    from number_token import number_token
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc;vp=doc.Views.ActiveView.ActiveViewport
    if list(doc.Objects)or doc.UndoRecordingIsActive:raise ValueError('surface curve array requires an empty owned document at idle')
    old_plane=vp.GetConstructionPlane();old_view=R.DocObjects.ViewportInfo(vp);old_tolerance=doc.ModelAbsoluteTolerance
    plane=G.Plane(G.Point3d(0.,0.,5.),G.Vector3d.ZAxis)
    if op['scene']=='Tilted':plane=G.Plane(G.Point3d(3.,-4.,5.),G.Vector3d(.6,.8,0.),G.Vector3d(-.48,.36,.8))
    if op['scene']=='Cylinder':surface=G.Cylinder(G.Circle(G.Plane.WorldXY,10.),20.).ToNurbsSurface()
    elif op['scene']=='Sphere':surface=G.Sphere(G.Point3d.Origin,10.).ToNurbsSurface()
    elif op['scene']=='Warp':
        surface=G.NurbsSurface.Create(3,False,3,3,3,3)
        for v in range(3):
            for u in range(3):surface.Points.SetControlPoint(u,v,G.ControlPoint(G.Point3d(6.*u,6.*v,5.+1.5*u*v+.5*u*u)))
        for knots in[surface.KnotsU,surface.KnotsV]:
            for i,n in enumerate([0.,0.,1.,1.]):knots[i]=n
    else:surface=G.PlaneSurface(plane,G.Interval(-20.,20.),G.Interval(-20.,20.)).ToNurbsSurface()
    if op['path']=='IsoU':path=surface.IsoCurve(0,surface.Domain(1).ParameterAt(.35))
    elif op['path']=='IsoV':
        path=surface.IsoCurve(1,surface.Domain(0).ParameterAt(.27))
        if op['scene']=='Sphere':short=path.Trim(G.Interval(path.Domain.ParameterAt(.1),path.Domain.ParameterAt(.9)));path.Dispose();path=short
    elif op['path']=='Circle':path=G.Circle(plane,5.).ToNurbsCurve()
    elif op['path']=='Nurbs':path=G.Curve.CreateControlPointCurve([plane.PointAt(x,y)for x,y in[(0.,0.),(3.,4.),(7.,-3.),(10.,5.)]],3)
    elif op['path']=='Polyline':path=G.PolylineCurve([plane.PointAt(x,y)for x,y in[(0.,0.),(4.,0.),(4.,4.),(10.,4.)]])
    else:path=G.LineCurve(plane.Origin,plane.PointAt(10.,0.)if op['path']=='LineX'else plane.PointAt(0.,10.))
    if op['scene']=='OffSurface':path.Transform(G.Transform.Translation(0.,0.,1.))
    if op['reverse_path']:path.Reverse()
    source_ids=[];rail=target=None;groups=[]
    # Rhino hides deleted group names. Journal names while publicly visible,
    # including controlled seeds, so later scenes retain their real input state.
    used_names=host.setdefault('_array_curve_surface_used_names',set())
    def snapshot():
        rows=[]
        for o in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
            g=o.Geometry
            if isinstance(g,G.PointCloud):geometry=dict(kind='PointCloud',points=[host['_xyz'](p.Location)for p in g])
            elif isinstance(g,G.Curve):geometry=dict(kind=g.GetType().Name,curve=host['_nurbs_curve_definition'](g))
            elif isinstance(g,G.Brep):geometry=dict(kind='Brep',surface=host['_nurbs_surface_definition'](g.Surfaces[0]),reversed=bool(g.Faces[0].OrientationIsReversed))
            else:raise ValueError('unsupported surface curve array object')
            rows.append(dict(role='source'if o.Id in source_ids else'rail'if o.Id==rail else'target'if o.Id==target else'output',source=source_ids.index(o.Id)if o.Id in source_ids else None,selected=bool(o.IsSelected(False)),name=o.Attributes.Name,groups=list(o.Attributes.GetGroupList()or[]),geometry=geometry))
        used_names.update(g.Name for g in doc.Groups if not g.IsDeleted and g.Name)
        return dict(objects=rows,groups=[dict(index=g.Index,name=g.Name)for g in doc.Groups if not g.IsDeleted],used_names=sorted(used_names),group_table=[dict(index=i,name=doc.Groups.GroupName(i),deleted=bool(doc.Groups.IsDeleted(i)))for i in range(doc.Groups.Count)])
    point=lambda p:'w'+','.join(number_token(v)for v in p)
    try:
        doc.ModelAbsoluteTolerance=1e-6;origin,x,y=op['cplane'];vp.SetConstructionPlane(G.Plane(G.Point3d(*origin),G.Vector3d(*x),G.Vector3d(*y)))
        record=doc.BeginUndoRecord('Owned surface curve array scene')
        try:
            for name in op['seed_names']:
                index=doc.Groups.Add(name)
                if index<0:raise ValueError('failed owned group-name seed')
                used_names.add(name)
                doc.Groups.Delete(index)
            for i in range(op['source_count']):
                attributes=R.DocObjects.ObjectAttributes();attributes.Name='Array source '+str(i);cloud=G.PointCloud([G.Point3d(p[0]+3.*i,p[1]+2.*i,p[2])for p in[[0.,0.,0.],[1.,0.,0.],[0.,1.,0.],[0.,0.,1.],[2.,1.,.5]]]);source_ids.append(doc.Objects.AddPointCloud(cloud,attributes));cloud.Dispose()
            if op['grouped']:
                group=doc.Groups.Add('Array group',source_ids);groups.append(group)
            attributes=R.DocObjects.ObjectAttributes();attributes.Name='Array rail';rail=doc.Objects.AddCurve(path,attributes)
            attributes=R.DocObjects.ObjectAttributes();attributes.Name='Array surface';brep=surface.ToBrep()
            if op['reverse_face']:brep.Flip()
            target=doc.Objects.AddBrep(brep,attributes);brep.Dispose()
        finally:doc.EndUndoRecord(record)
        if not op['postselect']:
            for i in source_ids:doc.Objects.Select(i)
        doc.ClearUndoRecords(True);before=snapshot();crcs=[doc.Objects.FindId(i).Geometry.DataCRC(0)for i in[rail,target]]
        actions=[];prompts=[]
        def action(value,prefix):actions.append(value);prompts.append([prefix]if isinstance(prefix,(str,type(u'')))else prefix)
        if op['postselect']:
            for i in range(len(source_ids)):action(dict(curve=2+i),'Select objects to array')
            action('Enter','Select objects to array')
        action(op['base_point'],'Base point')
        pick=dict(curve=0)
        if op['pick_point']is not None:pick['pick']=op['pick_point']
        if op['subcurve']is None:action(pick,'Select a curve on a surface')
        else:
            action('SubCrv','Select a curve on a surface');action(pick,'Select curve to temporarily shorten')
            action(host['_xyz'](path.PointAt(path.Domain.ParameterAt(op['subcurve'][0]))),'Start of curve')
            action(host['_xyz'](path.PointAt(path.Domain.ParameterAt(op['subcurve'][1]))),'Current length')
        if op['finish']=='CancelReference':action('Cancel','Select the surface')
        else:
            action(dict(curve=1),'Select the surface')
            def placement(mode,value):
                if mode=='Divide':action('Divide','Position objects or distance from last');action('Enter'if value is None else value,'Number of objects')
                elif mode=='Multiple':
                    action('Multiple','Position objects or distance from last');action('Enter'if value[0]is None else value[0],'Number of copies')
                    if value[0]!=1:action('Enter'if value[1]is None else value[1],'Distance between copies')
                else:
                    for p in value:action(p,'Position objects or distance from last')
            if op['mode']=='Sequence':
                modes={'divide':'Divide','multiple':'Multiple','point':'Positions','distance':'Distances'}
                for step in op['value']:
                    key=next(iter(step));value=step[key];placement(modes[key],[value]if key in('point','distance')else value)
            else:placement(op['mode'],op['value'])
            if op['finish']=='EnterCancel':action('Enter','Position objects or distance from last')
            action('Cancel',['Position objects or distance from last','Number of objects','Number of copies','Distance between copies'])
        marker='Owned surface curve array '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker);host['_record_progress'](op['id']+' surface curve array input');traces=[]
        def execute():
            from conic_getter_input import drive
            result,trace=drive(dict(op,actions=actions,action_prompts=prompts),host,[rail,target]+source_ids,command='ArrayCrvOnSrf');traces.append(trace);return result
        success,after,events=observe_command(R.Commands.Command,'ArrayCrvOnSrf',execute,snapshot,lambda:[],True)
        after_script=snapshot();history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip();R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot();undo_history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()[len(history):]
        return dict(success=success,actions=actions,action_prompts=prompts,input_trace=traces[0],before=before,after=after,after_script=after_script,undo=undo,redo=redo,events=events,history=history,undo_history=undo_history,guides_unchanged=all(doc.Objects.FindId(i).Geometry.DataCRC(0)==crc for i,crc in zip([rail,target],crcs))),0
    finally:
        for o in list(doc.Objects):doc.Objects.Delete(o.Id,True)
        for group in list(doc.Groups):
            if not group.IsDeleted:doc.Groups.Delete(group.Index)
        surface.Dispose();path.Dispose();vp.SetViewProjection(old_view,False);old_view.Dispose();vp.SetConstructionPlane(old_plane);doc.ModelAbsoluteTolerance=old_tolerance
