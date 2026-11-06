"""Observe public SrfPt geometry and atomic document policy in an owned job."""
if __package__:
    from .srf_pt_cases import validate,recipe
else:
    from srf_pt_cases import validate,recipe

def run(op,host,iterations,tolerance):
    validate(op)
    from join_probe import observe_command
    from conic_getter_input import drive
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects) or R.Commands.Command.InCommand():raise ValueError('empty owned idle document required')
    vp=doc.Views.ActiveView.ActiveViewport
    old=(doc.Layers.CurrentLayerIndex,vp.ConstructionPlane(),doc.ModelAbsoluteTolerance,doc.ModelRelativeTolerance,doc.ModelAngleToleranceRadians)
    case=recipe(op['case']);layers=[];source=group=None
    def snapshot():
        rows=[]
        for obj in sorted(list(doc.Objects),key=lambda o:int(o.RuntimeSerialNumber)):
            a,g=obj.Attributes,obj.Geometry
            row=dict(source=obj.Id==source,guide=False,selected=int(obj.IsSelected(False)),native_type=g.GetType().Name,native_valid=bool(g.IsValid),
                attributes=dict(name=a.Name or '',layer=doc.Layers[a.LayerIndex].Name,color_source=str(a.ColorSource),
                    color=[int(a.ObjectColor.R),int(a.ObjectColor.G),int(a.ObjectColor.B)],groups=[doc.Groups.GroupName(i) for i in (a.GetGroupList() or [])]))
            if obj.Id==source:row['point']=host['_xyz'](g.Location)
            else:
                if not isinstance(g,G.Brep) or g.Faces.Count!=1:raise ValueError('unexpected SrfPt output')
                surface=g.Faces[0].UnderlyingSurface()
                row.update(face_count=g.Faces.Count,surfaces=[host['_nurbs_surface_definition'](surface)],
                    singular=[bool(surface.IsSingular(i)) for i in range(4)],brep=host['_interchange_brep_record'](g,include_samples=False))
                if case.get('diagnostics'):
                    from srf_pt_diagnostics import record
                    row['sdk']=record(g,host)
            rows.append(row)
        return rows
    try:
        doc.ModelAbsoluteTolerance=tolerance['absolute'];doc.ModelRelativeTolerance=tolerance['relative'];doc.ModelAngleToleranceRadians=tolerance['angular']
        frame=case.get('construction_plane',[[0,0,0],[1,0,0],[0,1,0]])
        vp.SetConstructionPlane(G.Plane(G.Point3d(*frame[0]),G.Vector3d(*frame[1]),G.Vector3d(*frame[2])))
        for name in ['Plane sources','Plane output']:
            layer=R.DocObjects.Layer();layer.Name=name;layers.append(doc.Layers.Add(layer));layer.Dispose()
        a=R.DocObjects.ObjectAttributes();a.Name='sentinel';a.LayerIndex=layers[0]
        a.ObjectColor=S.Drawing.Color.FromArgb(12,34,56);a.ColorSource=R.DocObjects.ObjectColorSource.ColorFromObject
        source=doc.Objects.AddPoint(G.Point3d(50,60,70),a);a.Dispose()
        group=doc.Groups.Add('Plane source group',[source]);doc.Objects.Select(source)
        doc.Layers.SetCurrentLayerIndex(layers[1],True);doc.ClearUndoRecords(True);before=snapshot()
        marker='Owned SrfPt '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker);traces=[]
        def invoke():
            result,trace=drive(dict(id=op['id'],actions=case['actions']),host,[],'SrfPt');traces.append(trace);return result
        success,created,events=observe_command(R.Commands.Command,'SrfPt',invoke,snapshot,lambda:[],True)
        after=snapshot();history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        if R.Commands.Command.InCommand():raise ValueError('SrfPt did not return idle')
        plane=vp.ConstructionPlane()
        if success:
            R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
        else:undo,redo=before,after
        return dict(success=success,before=before,created=created,after=after,undo=undo,redo=redo,history=history,events=events,
            macro='_SrfPt _Pause',input_trace=traces,construction_plane=frame,returned_idle=True,
            model_tolerance=tolerance,cplane_after=[host['_xyz'](plane.Origin),host['_xyz'](plane.XAxis),host['_xyz'](plane.YAxis)]),0
    finally:
        R.RhinoApp.RunScript('!',False)
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        doc.ClearUndoRecords(True)
        if group is not None:doc.Groups.Delete(group)
        doc.Layers.SetCurrentLayerIndex(old[0],True);vp.SetConstructionPlane(old[1])
        doc.ModelAbsoluteTolerance,doc.ModelRelativeTolerance,doc.ModelAngleToleranceRadians=old[2:]
        for index in reversed(layers):doc.Layers.Delete(index,True)
