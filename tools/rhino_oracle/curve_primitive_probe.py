"""Public curve primitive command execution and exact history witnesses."""
def run(op,host,iterations,tolerance,command,driver=None):
    if iterations!=1:raise ValueError('curve primitive capture needs one iteration')
    from join_probe import observe_command
    R=host['Rhino'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc;vp=doc.Views.ActiveView.ActiveViewport
    before=set(obj.Id for obj in doc.Objects);saved=vp.GetConstructionPlane();guides=[];geometries=[]
    edge_input=any(isinstance(a,dict)and'edge'in a for a in op['actions'])
    old_view=R.DocObjects.ViewportInfo(vp)if edge_input or driver else None
    try:
        if 'construction_plane'in op:
            origin,x,y=op['construction_plane'];plane=G.Plane(G.Point3d(*origin),G.Vector3d(*x),G.Vector3d(*y))
            if not plane.IsValid:raise ValueError('invalid rectangle frame')
            vp.SetConstructionPlane(plane)
        else:vp.SetConstructionPlane(G.Plane.WorldXY)
        for source in ([op['curve']]if'curve'in op else[])+op.get('curves',[]):
            geometry=host['_object_source'](source,tolerance);geometries.append(geometry)
            guides.append(doc.Objects.AddSurface(geometry)if isinstance(geometry,G.Surface)else doc.Objects.AddBrep(geometry)if isinstance(geometry,G.Brep)else doc.Objects.AddCurve(geometry))
        if edge_input or driver:
            vp.SetProjection(R.Display.DefinedViewportProjection.Top,'Owned conic edge input',False)
            bounds=G.BoundingBox.Empty
            for g in geometries:bounds.Union(g.GetBoundingBox(True))
            bounds.Inflate(5.);
            if edge_input:vp.SetCameraDirection(G.Vector3d(-1.,1.,-1.),True);vp.CameraUp=G.Vector3d(0.,0.,1.)
            vp.ZoomBoundingBox(bounds);vp.SetConstructionPlane(plane if'construction_plane'in op else G.Plane.WorldXY);doc.Views.Redraw()
        guide_crcs=[doc.Objects.FindId(key).Geometry.DataCRC(0)for key in guides]
        guide=guides[0]if guides else None
        doc.Objects.UnselectAll()
        if guide and op.get('preselect'):doc.Objects.Select(guide,True)
        def curve_value(curve):
            pieces=list(curve.DuplicateSegments())if isinstance(curve,G.PolyCurve)else[curve.DuplicateCurve()]
            try:
                return dict(curve=host['_nurbs_curve_definition'](curve),segments=[host['_nurbs_curve_definition'](p)for p in pieces],length=float(curve.GetLength(tolerance["relative"])if command=="Conic"else curve.GetLength()),closed=bool(curve.IsClosed))
            finally:
                for p in pieces:p.Dispose()
        def snapshot():
            def output(obj):
                value = dict(point=host['_xyz'](obj.Geometry.Location)) if isinstance(obj.Geometry,G.Point) else curve_value(obj.Geometry)
                return dict(selected=bool(obj.IsSelected(False)),**value)
            return dict(outputs=[output(obj)for obj in doc.Objects if obj.Id not in before and obj.Id not in guides],guide_unchanged=all(doc.Objects.FindId(key).Geometry.DataCRC(0)==crc for key,crc in zip(guides,guide_crcs)))
        def macro_for(actions):
            tokens=['_'+command]
            for action in actions:
                if isinstance(action,dict):
                    if 'hover'in action:continue
                    if 'edge'in action:tokens.append('w'+','.join(repr(float(v))for v in action['pick']))
                    else:tokens.extend(["'_-SelID",str(guides[action['curve']])])
                    continue
                tokens.append('_'+action if isinstance(action,str)else'w'+','.join(repr(float(v))for v in action)if isinstance(action,list)else repr(float(action)))
                if action=='AroundCurve'and guide and not op.get('preselect'):tokens.extend(["'_-SelID",str(guide)])
            return ' '.join(tokens)
        if 'setup'in op:
            setup_before=set(obj.Id for obj in doc.Objects)
            setup_macro=macro_for(op['setup'])
            if not R.RhinoApp.RunScript(setup_macro,True):raise ValueError('curve setup failed')
            for obj in list(doc.Objects):
                if obj.Id not in setup_before:doc.Objects.Delete(obj.Id,True)
            doc.Objects.UnselectAll()
        tokens=[macro_for(op['actions'])]
        if op.get('cancel_tail'):tokens+=['_Cancel','_Cancel']
        macro=' '.join(tokens);marker='Owned '+command+' '+str(host['System'].Guid.NewGuid());R.RhinoApp.WriteLine(marker);host['_record_progress'](op['id']+' macro: '+macro)
        doc.ClearUndoRecords(True);seed=snapshot()
        input_trace=[]
        def execute():
            if driver is None:return R.RhinoApp.RunScript(macro,True)
            returned,trace=driver(guides);input_trace.append(trace);return returned
        if driver is not None:macro='_'+command+' _Pause'
        ok,after,events=observe_command(R.Commands.Command,command,execute,snapshot,lambda:[],True)
        result=dict(command_returned=bool(ok),before=seed,after=after,after_script=snapshot(),events=events,macro=macro,history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1].strip())
        if input_trace:result['input_trace']=input_trace[0]
        result['undo_returned']=bool(R.RhinoApp.RunScript('_Undo',False));result['undo']=snapshot()
        result['redo_returned']=bool(R.RhinoApp.RunScript('_Redo',False));result['redo']=snapshot()
        return result,0
    finally:
        for obj in list(doc.Objects):
            if obj.Id not in before:doc.Objects.Delete(obj.Id,True)
        for geometry in geometries:geometry.Dispose()
        if old_view is not None:vp.SetViewProjection(old_view,False);old_view.Dispose()
        vp.SetConstructionPlane(saved)
