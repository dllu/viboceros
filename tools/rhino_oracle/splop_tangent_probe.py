"""Public Splop tangent references with acknowledged, owned curve-contact input."""
try:
    from .circle_around_probe import validate as tangent_validate
except (ImportError,ValueError):
    from circle_around_probe import validate as tangent_validate

def validate(op):
    required={'op','id','curves','actions','construction_plane','copy','rigid','flip','surface','finish'}
    if not isinstance(op,dict)or set(op)!=required or op['op']!='splop_tangent':raise ValueError('invalid Splop tangent reference')
    if any(type(op[k])is not bool for k in('copy','rigid','flip'))or op['surface']not in('Plane','Cylinder')or op['finish']not in('Complete','CancelReference','CancelCopies'):raise ValueError('invalid tangent controls')
    if op['finish']=='CancelCopies'and not op['copy']:raise ValueError('cancelled copy batch needs Copy')
    if not op['actions']or op['actions'][0]!='Tangent':raise ValueError('reference must start Tangent')
    tangent_validate(dict(op='circle_tangent',id=op['id'],curves=op['curves'],actions=op['actions'],construction_plane=op['construction_plane'],live_getters=True))
    if any(source.get('type')in('point','box_brep')for source in op['curves']):raise ValueError('curve or single-surface guides required')

def run(op,host,iterations,tolerance):
    validate(op)
    if iterations!=1:raise ValueError('one owned tangent command required')
    from join_probe import observe_command
    from conic_getter_input import drive
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc;vp=doc.Views.ActiveView.ActiveViewport
    if list(doc.Objects)or doc.UndoRecordingIsActive:raise ValueError('tangent references require an empty owned document at idle')
    old_plane=vp.GetConstructionPlane();old_view=R.DocObjects.ViewportInfo(vp);old_tolerance=doc.ModelAbsoluteTolerance
    origin,x,y=op['construction_plane'];plane=G.Plane(G.Point3d(*origin),G.Vector3d(*x),G.Vector3d(*y));vp.SetConstructionPlane(plane);doc.ModelAbsoluteTolerance=1e-6
    surface=G.PlaneSurface(G.Plane(G.Point3d(0.,0.,5.),G.Vector3d.ZAxis),G.Interval(-20.,20.),G.Interval(-20.,20.)).ToNurbsSurface()if op['surface']=='Plane'else G.Cylinder(G.Circle(G.Plane.WorldXY,20.),20.).ToNurbsSurface()
    uv=[surface.Domain(0).ParameterAt(.17),surface.Domain(1).ParameterAt(.61)];ok,frame=surface.FrameAt(*uv)
    if not ok:raise ValueError('unavailable drop frame')
    source=target=None;guides=[];owned=[];traces=[]
    def snapshot():
        rows=[]
        for o in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
            if o.Id==target:continue
            g=o.Geometry
            if isinstance(g,G.PointCloud):geometry=dict(kind='PointCloud',points=[host['_xyz'](p.Location)for p in g])
            elif isinstance(g,G.Curve):geometry=dict(kind=g.GetType().Name,curve=host['_nurbs_curve_definition'](g))
            elif isinstance(g,G.Brep):geometry=dict(kind='Brep',surface=host['_nurbs_surface_definition'](g.Surfaces[0]))
            else:raise ValueError('unsupported tangent guide')
            rows.append(dict(role='source'if o.Id==source else'guide'if o.Id in guides else'output',guide=guides.index(o.Id)if o.Id in guides else None,selected=bool(o.IsSelected(False)),name=o.Attributes.Name,grips_on=bool(o.GripsOn),grips=[],geometry=geometry))
        return dict(objects=rows,target_selected=bool(doc.Objects.FindId(target).IsSelected(False)))
    try:
        record=doc.BeginUndoRecord('Owned Splop tangent scene')
        try:
            attributes=R.DocObjects.ObjectAttributes();attributes.Name='Splop source';cloud=G.PointCloud([G.Point3d(*p)for p in[[0.,0.,0.],[1.,0.,0.],[0.,1.,0.],[0.,0.,1.],[2.,1.,.5]]]);owned.append(cloud);source=doc.Objects.AddPointCloud(cloud,attributes)
            target=doc.Objects.AddSurface(surface)
            attributes=R.DocObjects.ObjectAttributes();attributes.Name='Reference guide'
            for description in op['curves']:
                geometry=host['_object_source'](description,tolerance);owned.append(geometry)
                if isinstance(geometry,G.Curve):
                    if not isinstance(geometry,G.LineCurve):geometry=geometry.ToNurbsCurve();owned.append(geometry)
                    guides.append(doc.Objects.AddCurve(geometry,attributes))
                elif isinstance(geometry,G.Surface):guides.append(doc.Objects.AddSurface(geometry,attributes))
                elif isinstance(geometry,G.Brep)and geometry.Faces.Count==1:guides.append(doc.Objects.AddBrep(geometry,attributes))
                else:raise ValueError('tangent references need curves or single surface guides')
        finally:doc.EndUndoRecord(record)
        vp.SetProjection(R.Display.DefinedViewportProjection.Top,'Owned Splop tangent input',False);vp.ZoomExtents();doc.Views.Redraw();doc.Objects.Select(source);doc.ClearUndoRecords(True)
        before=snapshot();crcs=[doc.Objects.FindId(i).Geometry.DataCRC(0)for i in guides+[target]]
        actions=list(op['actions'])
        if op['finish']=='CancelReference':
            if actions[-1]!='Cancel':actions+=['Cancel']
        else:
            actions += [dict(curve=len(guides)), 'Copy='+('Yes'if op['copy']else'No'),'Rigid='+('Yes'if op['rigid']else'No'),'Flip='+('Yes'if op['flip']else'No'),host['_xyz'](frame.Origin),host['_xyz'](frame.Origin+frame.XAxis*4.)]
            if op['copy']:actions+=['Cancel'if op['finish']=='CancelCopies'else'Enter']
        driven=dict(op,actions=actions)
        marker='Owned Splop tangent '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker);host['_record_progress'](op['id']+' tangent input')
        def execute():
            result,trace=drive(driven,host,guides+[target],command='Splop');traces.append(trace);return result
        success,after,events=observe_command(R.Commands.Command,'Splop',execute,snapshot,lambda:[],True)
        after_script=snapshot();history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot();undo_history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()[len(history):]
        return dict(success=success,before=before,after=after,after_script=after_script,undo=undo,redo=redo,events=events,history=history,undo_history=undo_history,input_trace=traces[0],driven_actions=actions,guides_unchanged=all(doc.Objects.FindId(i).Geometry.DataCRC(0)==crc for i,crc in zip(guides+[target],crcs)),surface=host['_nurbs_surface_definition'](surface),uv=uv,frame=dict(origin=host['_xyz'](frame.Origin),x=host['_xyz'](frame.XAxis),y=host['_xyz'](frame.YAxis))),0
    finally:
        for o in list(doc.Objects):doc.Objects.Delete(o.Id,True)
        for geometry in owned:geometry.Dispose()
        surface.Dispose();vp.SetViewProjection(old_view,False);old_view.Dispose();vp.SetConstructionPlane(old_plane);doc.ModelAbsoluteTolerance=old_tolerance
