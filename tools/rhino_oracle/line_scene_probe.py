"""Owned line-construction scene and document/history observations."""

def run_case(op,host,iterations,tolerance,case,command,capture_recall=True):
    from join_probe import observe_command
    from conic_getter_input import drive
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects) or R.Commands.Command.InCommand():raise ValueError('empty owned idle document required')
    vp=doc.Views.ActiveView.ActiveViewport
    old=(doc.Layers.CurrentLayerIndex,vp.ConstructionPlane(),doc.ModelAbsoluteTolerance,doc.ModelRelativeTolerance,doc.ModelAngleToleranceRadians)
    layers=[];source=group=None
    def snapshot():
        rows=[]
        for obj in sorted(list(doc.Objects),key=lambda o:int(o.RuntimeSerialNumber)):
            a,g=obj.Attributes,obj.Geometry
            row=dict(source=obj.Id==source,guide=False,selected=int(obj.IsSelected(False)),native_type=g.GetType().Name,native_valid=bool(g.IsValid),
                attributes=dict(name=a.Name or '',layer=doc.Layers[a.LayerIndex].Name,color_source=str(a.ColorSource),
                    color=[int(a.ObjectColor.R),int(a.ObjectColor.G),int(a.ObjectColor.B)],groups=[doc.Groups.GroupName(i) for i in (a.GetGroupList() or [])]))
            if isinstance(g,G.Point):row['point']=host['_xyz'](g.Location)
            elif isinstance(g,G.LineCurve):row['curve']=host['_nurbs_curve_definition'](g)
            else:raise ValueError('unexpected Lines output')
            rows.append(row)
        return rows
    try:
        tolerance=dict(tolerance);tolerance['absolute']=case.get('absolute_tolerance',tolerance['absolute'])
        doc.ModelAbsoluteTolerance=tolerance['absolute'];doc.ModelRelativeTolerance=tolerance['relative'];doc.ModelAngleToleranceRadians=tolerance['angular']
        frame=case.get('construction_plane',[[0,0,0],[1,0,0],[0,1,0]])
        vp.SetConstructionPlane(G.Plane(G.Point3d(*frame[0]),G.Vector3d(*frame[1]),G.Vector3d(*frame[2])))
        for name in ['Plane sources','Plane output']:
            layer=R.DocObjects.Layer();layer.Name=name;layers.append(doc.Layers.Add(layer));layer.Dispose()
        a=R.DocObjects.ObjectAttributes();a.Name='sentinel';a.LayerIndex=layers[0]
        a.ObjectColor=S.Drawing.Color.FromArgb(12,34,56);a.ColorSource=R.DocObjects.ObjectColorSource.ColorFromObject
        source=doc.Objects.AddPoint(G.Point3d(50,60,70),a);a.Dispose()
        group=doc.Groups.Add('Plane source group',[source]);doc.Objects.Select(source)
        doc.Layers.SetCurrentLayerIndex(layers[1],True);doc.ClearUndoRecords(True)
        if case.get('pending_redo'):
            R.RhinoApp.RunScript('_Point w90,91,92',False);R.RhinoApp.RunScript('_Undo',False)
        before=snapshot();marker='Owned '+command+' '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker);traces=[]
        def invoke():
            if case.get('macro'):
                import os
                path=os.path.join(os.path.dirname(host['__file__']),'owned Lines macro.txt')
                with open(path,'wb') as stream:stream.write(case['macro'].encode('utf-8'))
                return R.RhinoApp.RunScript('_-ReadCommandFile "'+path.replace('/','\\')+'"',True)
            result,trace=drive(dict(id=op['id'],actions=case['actions']),host,[],command);traces.append(trace);return result
        success,created,events=observe_command(R.Commands.Command,'ReadCommandFile' if case.get('macro') else command,invoke,snapshot,lambda:[],True)
        after=snapshot();history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        if R.Commands.Command.InCommand():raise ValueError('Lines did not return idle')
        last_selection=None
        if capture_recall:
            selected=[o.Id for o in doc.Objects if o.IsSelected(False)]
            R.RhinoApp.RunScript('_SelLast',False);last_selection=snapshot()
            doc.Objects.UnselectAll()
            for id in selected:doc.Objects.Select(id)
        if before!=after:
            R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
        else:undo,redo=before,after
        pending_redo=None
        if case.get('pending_redo'):
            R.RhinoApp.RunScript('_Redo',False);pending_redo=snapshot()
        return dict(success=success,before=before,created=created,after=after,undo=undo,redo=redo,history=history,events=events,
            input_trace=traces,construction_plane=frame,returned_idle=True,model_tolerance=tolerance,last_selection=last_selection,pending_redo=pending_redo),0
    finally:
        R.RhinoApp.RunScript('!',False)
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        doc.ClearUndoRecords(True)
        if group is not None:doc.Groups.Delete(group)
        doc.Layers.SetCurrentLayerIndex(old[0],True);vp.SetConstructionPlane(old[1])
        doc.ModelAbsoluteTolerance,doc.ModelRelativeTolerance,doc.ModelAngleToleranceRadians=old[2:]
        for index in reversed(layers):doc.Layers.Delete(index,True)
