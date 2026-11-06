"""Observe command-file geometry and atomic document policy in an owned job."""
if __package__:
    from .command_file_cases import validate,recipe
else:
    from command_file_cases import validate,recipe

def run(op,host,iterations,tolerance):
    validate(op)
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects) or R.Commands.Command.InCommand():raise ValueError('empty owned idle document required: iterations=%r objects=%r in_command=%r' % (iterations,[o.Geometry.GetType().Name for o in doc.Objects],R.Commands.Command.InCommand()))
    vp=doc.Views.ActiveView.ActiveViewport
    old=(doc.Layers.CurrentLayerIndex,vp.ConstructionPlane(),doc.ModelAbsoluteTolerance,doc.ModelRelativeTolerance,doc.ModelAngleToleranceRadians)
    case=recipe(op['case']);command='ReadCommandFile';layers=[];source=group=None
    original_layers=set(l.Id for l in doc.Layers)
    def layer_table():
        return [dict(name=l.Name,visible=bool(l.IsVisible),locked=bool(l.IsLocked),color=[int(l.Color.R),int(l.Color.G),int(l.Color.B)],current=l.Index==doc.Layers.CurrentLayerIndex) for l in doc.Layers if not l.IsDeleted]
    def snapshot():
        rows=[]
        for obj in sorted(list(doc.Objects),key=lambda o:int(o.RuntimeSerialNumber)):
            a,g=obj.Attributes,obj.Geometry
            row=dict(source=obj.Id==source,guide=False,selected=int(obj.IsSelected(False)),native_type=g.GetType().Name,native_valid=bool(g.IsValid),
                attributes=dict(name=a.Name or '',layer=doc.Layers[a.LayerIndex].Name,color_source=str(a.ColorSource),
                    color=[int(a.ObjectColor.R),int(a.ObjectColor.G),int(a.ObjectColor.B)],groups=[doc.Groups.GroupName(i) for i in (a.GetGroupList() or [])]))
            if isinstance(g,G.Point):row['point']=host['_xyz'](g.Location)
            elif isinstance(g,G.Curve):row['curve']=host['_nurbs_curve_definition'](g)
            elif isinstance(g,G.PointCloud):row['points']=[host['_xyz'](p) for p in g.GetPoints()]
            else:
                if not isinstance(g,G.Brep):raise ValueError('unexpected point-grid output')
                row.update(face_count=g.Faces.Count,surfaces=[host['_nurbs_surface_definition'](f.UnderlyingSurface()) for f in g.Faces],
                    brep=host['_interchange_brep_record'](g,include_samples=False))
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
        doc.Layers.SetCurrentLayerIndex(layers[1],True);doc.ClearUndoRecords(True);before=snapshot();layers_before=layer_table()
        marker='Owned command file '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker);traces=[]
        import os
        path=os.path.join(os.path.dirname(host['__file__']),'owned command file.txt')
        text=case['text']
        if 'child' in case:
            child=os.path.join(os.path.dirname(host['__file__']),'owned child file.txt')
            with open(child,'wb') as stream:stream.write(case['child'].encode('utf-8'))
            text=text.replace('@CHILD@',child.replace('/','\\'))
        with open(path,'wb') as stream:stream.write(text.encode(case.get('encoding','utf-8')))
        macro='_-ReadCommandFile "'+path.replace('/','\\')+'"'
        def invoke():
            if 'actions' not in case:return R.RhinoApp.RunScript(macro,True)
            from conic_getter_input import drive
            result,trace=drive(dict(id=op['id'],actions=case['actions']),host,[],command,start_script=macro)
            traces.append(trace);return result
        events=[];results=[];errors=[];depth=[0]
        def began(sender,event):
            if event.CommandEnglishName==command:depth[0]+=1
        def ended(sender,event):
            try:
                name=event.CommandEnglishName;result=str(event.CommandResult)
                events.append(dict(name=name,result=result))
                if name==command:
                    depth[0]-=1
                    if depth[0]==0:results.append((result=='Success',snapshot()))
            except Exception as error:errors.append(str(error))
        R.Commands.Command.BeginCommand+=began;R.Commands.Command.EndCommand+=ended
        try:invoke()
        finally:
            R.Commands.Command.BeginCommand-=began;R.Commands.Command.EndCommand-=ended
        if errors or depth[0]!=0 or len(results)!=1:raise ValueError('command-file observer failed: %r %r %r'%(errors,depth,len(results)))
        success,created=results[0]

        after=snapshot();history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        if R.Commands.Command.InCommand():raise ValueError('command file did not return idle')
        plane=vp.ConstructionPlane();layers_after=layer_table();last_selection=None
        if case.get('last_selection'):
            selected=[o.Id for o in doc.Objects if o.IsSelected(False)]
            R.RhinoApp.RunScript('_SelLast',False);last_selection=snapshot()
            doc.Objects.UnselectAll()
            for id in selected:doc.Objects.Select(id)
        undo_steps=[];redo_steps=[]
        for _ in range(16):
            if snapshot()==before:break
            R.RhinoApp.RunScript('_Undo',False);undo_steps.append(snapshot())
        layers_undo=layer_table()
        for _ in undo_steps:
            R.RhinoApp.RunScript('_Redo',False);redo_steps.append(snapshot())
        undo=undo_steps[-1] if undo_steps else after;redo=redo_steps[-1] if redo_steps else after
        return dict(success=success,before=before,created=created,after=after,undo=undo,redo=redo,history=history,events=events,
            macro=macro,command=command,input_trace=traces,undo_steps=undo_steps,redo_steps=redo_steps,
            layers_before=layers_before,layers_after=layers_after,layers_undo=layers_undo,layers_redo=layer_table(),last_selection=last_selection,construction_plane=frame,returned_idle=True,
            model_tolerance=tolerance,cplane_after=[host['_xyz'](plane.Origin),host['_xyz'](plane.XAxis),host['_xyz'](plane.YAxis)]),0
    finally:
        R.RhinoApp.RunScript('!',False)
        for layer in list(doc.Layers):
            if layer.Id not in original_layers and not layer.IsDeleted:
                layer.IsLocked=False;layer.IsVisible=True;layer.CommitChanges()
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        doc.ClearUndoRecords(True)
        if group is not None:doc.Groups.Delete(group)
        doc.Layers.SetCurrentLayerIndex(old[0],True);vp.SetConstructionPlane(old[1])
        doc.ModelAbsoluteTolerance,doc.ModelRelativeTolerance,doc.ModelAngleToleranceRadians=old[2:]
        for layer in list(doc.Layers):
            if layer.Id not in original_layers:doc.Layers.Delete(layer.Index,True)
