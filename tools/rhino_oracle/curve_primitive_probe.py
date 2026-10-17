"""Public curve primitive command execution and exact history witnesses."""
def run(op,host,iterations,tolerance,command):
    if iterations!=1:raise ValueError('curve primitive capture needs one iteration')
    from join_probe import observe_command
    R=host['Rhino'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc;vp=doc.Views.ActiveView.ActiveViewport
    before=set(obj.Id for obj in doc.Objects);saved=vp.GetConstructionPlane();guides=[];geometries=[]
    try:
        if 'construction_plane'in op:
            origin,x,y=op['construction_plane'];plane=G.Plane(G.Point3d(*origin),G.Vector3d(*x),G.Vector3d(*y))
            if not plane.IsValid:raise ValueError('invalid rectangle frame')
            vp.SetConstructionPlane(plane)
        else:vp.SetConstructionPlane(G.Plane.WorldXY)
        for source in ([op['curve']]if'curve'in op else[])+op.get('curves',[]):
            geometry=host['_object_source'](source,tolerance);geometries.append(geometry);guides.append(doc.Objects.AddCurve(geometry))
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
            return dict(outputs=[dict(selected=bool(obj.IsSelected(False)),**curve_value(obj.Geometry))for obj in doc.Objects if obj.Id not in before and obj.Id not in guides],guide_unchanged=all(doc.Objects.FindId(key).Geometry.DataCRC(0)==geometry.DataCRC(0)for key,geometry in zip(guides,geometries)))
        def macro_for(actions):
            tokens=['_'+command]
            for action in actions:
                if isinstance(action,dict):
                    tokens.extend(["'_-SelID",str(guides[action['curve']])]);continue
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
        ok,after,events=observe_command(R.Commands.Command,command,lambda:R.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        result=dict(command_returned=bool(ok),before=seed,after=after,after_script=snapshot(),events=events,macro=macro,history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1].strip())
        result['undo_returned']=bool(R.RhinoApp.RunScript('_Undo',False));result['undo']=snapshot()
        result['redo_returned']=bool(R.RhinoApp.RunScript('_Redo',False));result['redo']=snapshot()
        return result,0
    finally:
        for obj in list(doc.Objects):
            if obj.Id not in before:doc.Objects.Delete(obj.Id,True)
        for geometry in geometries:geometry.Dispose()
        vp.SetConstructionPlane(saved)
