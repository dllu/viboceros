"""Public curve primitive command execution and exact history witnesses."""
def run(op,host,iterations,tolerance,command,driver=None,setup_driver=None):
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
            guides.append(doc.Objects.AddPoint(geometry.Location)if isinstance(geometry,G.Point)else doc.Objects.AddSurface(geometry)if isinstance(geometry,G.Surface)else doc.Objects.AddBrep(geometry)if isinstance(geometry,G.Brep)else doc.Objects.AddCurve(geometry))
        if edge_input or driver:
            vp.SetProjection(R.Display.DefinedViewportProjection.Top,'Owned conic edge input',False)
            bounds=G.BoundingBox.Empty
            for g in geometries:bounds.Union(g.GetBoundingBox(True))
            if command in('Catenary','Arc')and any(isinstance(a,dict)and'click'in a for a in op['actions']):
                for a in op['actions']:
                    if isinstance(a,list):bounds.Union(G.Point3d(*a))
                    elif isinstance(a,dict)and('click'in a or'hover'in a):bounds.Union(G.Point3d(*a.get('click',a.get('hover'))))
            bounds.Inflate(5.);
            if edge_input:vp.SetCameraDirection(G.Vector3d(-1.,1.,-1.),True);vp.CameraUp=G.Vector3d(0.,0.,1.)
            vp.ZoomBoundingBox(bounds);vp.SetConstructionPlane(plane if'construction_plane'in op else G.Plane.WorldXY);doc.Views.Redraw()
        if 'prior_point'in op:
            prior_before=set(o.Id for o in doc.Objects)
            if not R.RhinoApp.RunScript('_Point w'+','.join(repr(float(v))for v in op['prior_point']),False):raise ValueError('failed to establish prior point')
            for obj in list(doc.Objects):
                if obj.Id not in prior_before:doc.Objects.Delete(obj.Id,True)
        guide_crcs=[doc.Objects.FindId(key).Geometry.DataCRC(0)for key in guides]
        guide=guides[0]if guides else None
        doc.Objects.UnselectAll()
        if op.get('preselect_all'):
            for key in guides:doc.Objects.Select(key,True)
        elif guide and op.get('preselect'):doc.Objects.Select(guide,True)
        def curve_value(curve):
            pieces=list(curve.DuplicateSegments())if isinstance(curve,G.PolyCurve)else[curve.DuplicateCurve()]
            try:
                value=dict(curve=host['_nurbs_curve_definition'](curve),segments=[host['_nurbs_curve_definition'](p)for p in pieces],length=float(curve.GetLength(tolerance["relative"])if command=="Conic"or op.get('precise_length')else curve.GetLength()),closed=bool(curve.IsClosed))
                controls=value['curve']['control_points']
                if command=='Hyperbola'and value['length']==0. and value['curve']['degree']==2 and len(controls)==3 and controls[0]['weight']==controls[2]['weight']==1. and controls[1]['weight']<=-1.:
                    # The quadratic denominator has roots inside its domain.
                    # Zero is Rhino's failed GetLength query result, not an arc length.
                    value['length_query_failed']=True
                return value
            finally:
                for p in pieces:p.Dispose()
        def snapshot():
            def output(obj):
                value = dict(point=host['_xyz'](obj.Geometry.Location)) if isinstance(obj.Geometry,G.Point) else curve_value(obj.Geometry)
                return dict(selected=bool(obj.IsSelected(False)),**value)
            value=dict(outputs=[output(obj)for obj in doc.Objects if obj.Id not in before and obj.Id not in guides],guide_unchanged=all(doc.Objects.FindId(key).Geometry.DataCRC(0)==crc for key,crc in zip(guides,guide_crcs)))
            if op.get('guide_selection'):value['guide_selected']=[bool(doc.Objects.FindId(key).IsSelected(False))for key in guides]
            return value
        def macro_for(actions):
            tokens=['_'+command]
            for action in actions:
                if isinstance(action,dict):
                    if 'hover'in action or 'click'in action:continue
                    if 'relative'in action:tokens.append('r'+','.join(repr(float(v))for v in action['relative']));continue
                    if 'edge'in action:tokens.append('w'+','.join(repr(float(v))for v in action['pick']))
                    else:tokens.extend(["'_-SelID",str(guides[action['curve']])])
                    continue
                tokens.append('_'+action if isinstance(action,str)else'w'+','.join(repr(float(v))for v in action)if isinstance(action,list)else repr(float(action)))
                if action=='AroundCurve'and guide and not op.get('preselect')and not any(isinstance(a,dict)and'curve'in a for a in actions):tokens.extend(["'_-SelID",str(guide)])
            return ' '.join(tokens)
        setup_trace=None
        if 'setup'in op:
            setup_before=set(obj.Id for obj in doc.Objects)
            setup_macro=macro_for(op['setup'])
            if setup_driver is None:
                if not R.RhinoApp.RunScript(setup_macro,True):raise ValueError('curve setup failed')
            else:
                setup_ok,setup_trace=setup_driver(guides)
                if not setup_ok or setup_trace['errors'] or setup_trace['completed']!=setup_trace['expected']:raise ValueError('live curve setup failed')
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
        if op.get('curve_diagnostics'):
            result['curve_diagnostics']=[dict(valid=bool(obj.Geometry.IsValid),midpoint_valid=bool(obj.Geometry.PointAt(0.5*(obj.Geometry.Domain.T0+obj.Geometry.Domain.T1)).IsValid))for obj in doc.Objects if obj.Id not in before and obj.Id not in guides and isinstance(obj.Geometry,G.Curve)]
        if input_trace:result['input_trace']=input_trace[0]
        if setup_trace is not None:result['setup_input_trace']=setup_trace
        result['undo_returned']=bool(R.RhinoApp.RunScript('_Undo',False));result['undo']=snapshot()
        result['redo_returned']=bool(R.RhinoApp.RunScript('_Redo',False));result['redo']=snapshot()
        return result,0
    finally:
        for obj in list(doc.Objects):
            if obj.Id not in before:doc.Objects.Delete(obj.Id,True)
        for geometry in geometries:geometry.Dispose()
        if old_view is not None:vp.SetViewProjection(old_view,False);old_view.Dispose()
        vp.SetConstructionPlane(saved)
