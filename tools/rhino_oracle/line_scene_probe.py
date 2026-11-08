"""Owned line-construction scene and document/history observations."""

def run_case(op,host,iterations,tolerance,case,command,capture_recall=True,curve_observer=None,completion_observer=None,source_setup=None,capture_undo=False,sources_prepared=None,object_observer=None):
    from join_probe import observe_command
    from conic_getter_input import drive
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects) or R.Commands.Command.InCommand():raise ValueError('empty owned idle document required')
    vp=doc.Views.ActiveView.ActiveViewport
    old=(doc.Layers.CurrentLayerIndex,vp.ConstructionPlane(),doc.ModelAbsoluteTolerance,doc.ModelRelativeTolerance,doc.ModelAngleToleranceRadians)
    layers=[];source=group=None;guides=[];geometries=[];old_view=None;old_display=vp.DisplayMode
    def snapshot():
        rows=[]
        for obj in sorted(list(doc.Objects),key=lambda o:int(o.RuntimeSerialNumber)):
            a,g=obj.Attributes,obj.Geometry
            row=dict(source=obj.Id==source,guide=obj.Id in guides,selected=int(obj.IsSelected(False)),native_type=g.GetType().Name,native_valid=bool(g.IsValid),
                attributes=dict(name=a.Name or '',layer=doc.Layers[a.LayerIndex].Name,color_source=str(a.ColorSource),
                    color=[int(a.ObjectColor.R),int(a.ObjectColor.G),int(a.ObjectColor.B)],groups=[doc.Groups.GroupName(i) for i in (a.GetGroupList() or [])]))
            if isinstance(g,G.Point):row['point']=host['_xyz'](g.Location)
            elif isinstance(g,G.Curve):
                if row['native_valid']:
                    row['curve']=host['_nurbs_curve_definition'](g)
                    if curve_observer is not None:row.update(curve_observer(g))
                else:
                    # A public command can succeed with a degenerate result.
                    # Preserve that evidence without trying NURBS conversion.
                    row['invalid_geometry']=True
                    row['curve_domain']=[float(g.Domain.T0),float(g.Domain.T1)] if g.Domain.IsValid else None
                    row['curve_endpoints']=[host['_xyz'](p) if p.IsValid else None for p in [g.PointAtStart,g.PointAtEnd]]
            elif isinstance(g,G.Brep):row.update(face_count=g.Faces.Count,surfaces=[host['_nurbs_surface_definition'](f.UnderlyingSurface())for f in g.Faces],brep=host['_interchange_brep_record'](g,include_samples=False))
            elif isinstance(g,G.Surface):row.update(face_count=1,surfaces=[host['_nurbs_surface_definition'](g)])
            else:raise ValueError('unexpected Lines output')
            if object_observer is not None: row.update(object_observer(obj))
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
        if case.get('curves'):
            old_view=R.DocObjects.ViewportInfo(vp)
            bounds=G.BoundingBox.Empty
            for i,definition in enumerate(case['curves']):
                if definition.get('type')=='normal_surface':
                    from move_normal_probe import geometry as normal_geometry
                    geometry=normal_geometry(definition['target'],host)
                else:geometry=host['_object_source'](definition,tolerance)
                geometries.append(geometry)
                attrs=R.DocObjects.ObjectAttributes();attrs.Name='guide'+str(i);attrs.LayerIndex=layers[0]
                guides.append(doc.Objects.AddBrep(geometry,attrs)if isinstance(geometry,G.Brep)else doc.Objects.AddSurface(geometry,attrs)if isinstance(geometry,G.Surface)else doc.Objects.AddCurve(geometry,attrs));attrs.Dispose();bounds.Union(geometry.GetBoundingBox(True))
            vp.SetProjection(getattr(R.Display.DefinedViewportProjection,case.get('view','Top')),'Owned Line reference picking',False)
            bounds.Inflate(5.);vp.ZoomBoundingBox(bounds)
            if case.get('shaded'):vp.DisplayMode=R.Display.DisplayModeDescription.FindByName('Shaded')
            vp.SetConstructionPlane(G.Plane(G.Point3d(*frame[0]),G.Vector3d(*frame[1]),G.Vector3d(*frame[2])));doc.Views.Redraw()
        group=doc.Groups.Add('Plane source group',[source]+guides)
        if sources_prepared is not None:sources_prepared(guides)
        doc.Objects.Select(source)
        doc.Layers.SetCurrentLayerIndex(layers[1],True);doc.ClearUndoRecords(True)
        if case.get('pending_redo'):
            R.RhinoApp.RunScript('_Point w90,91,92',False);R.RhinoApp.RunScript('_Undo',False)
        if source_setup is not None:source_setup(guides)
        before=snapshot();marker='Owned '+command+' '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker);traces=[]
        def invoke():
            if case.get('macro'):
                import os
                path=os.path.join(os.path.dirname(host['__file__']),'owned Lines macro.txt')
                with open(path,'wb') as stream:stream.write(case['macro'].encode('utf-8'))
                return R.RhinoApp.RunScript('_-ReadCommandFile "'+path.replace('/','\\')+'"',True)
            result,trace=drive(dict(op=op['op'],id=op['id'],actions=case['actions'],planar=case.get('planar',False)),host,guides,command);traces.append(trace);return result
        success,created,events=observe_command(R.Commands.Command,'ReadCommandFile' if case.get('macro') else command,invoke,snapshot,lambda:[],True)
        after=snapshot();completion=completion_observer()if completion_observer is not None else None
        history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        if R.Commands.Command.InCommand():raise ValueError('Lines did not return idle')
        last_selection=None
        if capture_recall:
            selected=[o.Id for o in doc.Objects if o.IsSelected(False)]
            R.RhinoApp.RunScript('_SelLast',False);last_selection=snapshot()
            doc.Objects.UnselectAll()
            for id in selected:doc.Objects.Select(id)
        geometry_changed=[dict(o,selected=0)for o in before]!=[dict(o,selected=0)for o in after]
        history_changed=geometry_changed;undo_observation=None
        if capture_undo:
            def history_command(name):
                mark='Owned history probe '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(mark)
                signals=[];errors=[]
                def changed(sender,event):
                    try:signals.append(dict(serial=int(event.UndoSerialNumber),command_id=str(event.CommandId),begin_undo=bool(event.IsBeginUndo),end_undo=bool(event.IsEndUndo),begin_redo=bool(event.IsBeginRedo),end_redo=bool(event.IsEndRedo)))
                    except Exception as error:errors.append(str(error))
                R.Commands.Command.UndoRedo += changed
                try:ok,objects,events=observe_command(R.Commands.Command,name,lambda:R.RhinoApp.RunScript('_'+name,True),snapshot,lambda:[],True)
                finally:R.Commands.Command.UndoRedo -= changed
                if errors:raise ValueError('UndoRedo event observer failed: '+str(errors))
                log=R.RhinoApp.CommandHistoryWindowText.split(mark,1)[1].strip()
                return ok,objects,events,log,signals
            def completed(signals,kind):
                begins=[s['serial']for s in signals if s['begin_'+kind]];ends=[s['serial']for s in signals if s['end_'+kind]]
                if begins!=ends or len(begins)>1:raise ValueError('incomplete UndoRedo event pair')
                return bool(begins)
            undo_ok,undo,undo_events,undo_log,undo_signals=history_command('Undo')
            undid=completed(undo_signals,'undo');redid=redo_ok=None;redo=after;redo_events=[];redo_log=None;redo_signals=[]
            if undid or capture_undo=='diagnostic':
                redo_ok,redo,redo_events,redo_log,redo_signals=history_command('Redo');redid=completed(redo_signals,'redo')
            if capture_undo!='diagnostic':
                history_changed=undid
                if undid and not redid:raise ValueError('observed Undo without matching Redo')
                if geometry_changed and not undid:raise ValueError('native edit has no Undo record')
            undo_observation=dict(undo_succeeded=undid,redo_succeeded=redid,geometry_changed=geometry_changed,undo_command_succeeded=undo_ok,redo_command_succeeded=redo_ok,undo_events=undo_events,redo_events=redo_events,undo_history=undo_log,redo_history=redo_log,undo_signals=undo_signals,redo_signals=redo_signals)
        elif history_changed:
            R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
        else:undo,redo=after,after
        pending_redo=None
        if case.get('pending_redo'):
            R.RhinoApp.RunScript('_Redo',False);pending_redo=snapshot()
        value=dict(success=success,before=before,created=created,after=after,undo=undo,redo=redo,history=history,events=events,
            input_trace=traces,history_changed=history_changed,construction_plane=frame,returned_idle=True,model_tolerance=tolerance,last_selection=last_selection,pending_redo=pending_redo)
        if completion is not None:value['completion_observation']=completion
        if undo_observation is not None:value['undo_observation']=undo_observation
        return value,0
    finally:
        R.RhinoApp.RunScript('!',False)
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        doc.ClearUndoRecords(True)
        for geometry in geometries:geometry.Dispose()
        if old_view is not None:vp.SetViewProjection(old_view,False);old_view.Dispose()
        vp.DisplayMode=old_display
        if group is not None:doc.Groups.Delete(group)
        doc.Layers.SetCurrentLayerIndex(old[0],True);vp.SetConstructionPlane(old[1])
        doc.ModelAbsoluteTolerance,doc.ModelRelativeTolerance,doc.ModelAngleToleranceRadians=old[2:]
        for index in reversed(layers):doc.Layers.Delete(index,True)
