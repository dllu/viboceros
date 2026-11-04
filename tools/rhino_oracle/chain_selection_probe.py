"""Public native chain selection and immutable source CRCs."""
if __package__:
    from .chain_selection_cases import validate,source
else:
    from chain_selection_cases import validate,source

def run(op,host,iterations,tolerance):
    validate(op)
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects) or R.Commands.Command.InCommand():raise ValueError('empty private idle document required')
    recipe=source(op['scene']);inputs=[];ids=[]
    try:
        R.RhinoApp.RunScript('_SelectionFilterNone',False)
        for row in recipe:
            pts=[G.Point3d(*p) for p in row['points']]
            if row['kind']=='line':geo=G.LineCurve(*pts)
            elif row['kind']=='arc':geo=G.ArcCurve(G.Arc(*pts))
            elif row['kind']=='circle':geo=G.ArcCurve(G.Circle(*pts))
            else:geo=G.Brep.CreateFromCornerPoints(*(pts+[doc.ModelAbsoluteTolerance]))
            if geo is None:raise ValueError('cannot create owned SelChain source')
            inputs.append(geo);key=doc.Objects.Add(geo);ids.append(key)
            if key==S.Guid.Empty:raise ValueError('cannot insert owned SelChain source')
        R.RhinoApp.RunScript('_SetView _World _Top',False);R.RhinoApp.RunScript('_Zoom _Extents',False)
        doc.Views.ActiveView.ActiveViewport.DisplayMode=R.Display.DisplayModeDescription.FindByName('Wireframe');doc.Views.Redraw()
        crc=[int(g.DataCRC(0)) for g in inputs]
        doc.ClearUndoRecords(True)
        R.RhinoApp.RunScript('_Point w40,50,60',False);R.RhinoApp.RunScript('_Undo',False)
        if len(list(doc.Objects))!=len(ids):raise ValueError('cannot seed owned redo')
        for i in op['pre']:doc.Objects.Select(ids[i])
        def snapshot():
            state=dict(whole=[i for i,k in enumerate(ids) if doc.Objects.FindId(k).IsSelected(False)],
                components=[dict(object=i,kind=str(c.ComponentIndexType),index=int(c.Index)) for i,k in enumerate(ids) for c in (doc.Objects.FindId(k).GetSelectedSubObjects() or [])],objects=len(list(doc.Objects)))
            if op.get('inspect'):
                state['highlighted_whole']=[i for i,k in enumerate(ids) if doc.Objects.FindId(k).IsHighlighted(False)]
                state['highlighted']=[dict(object=i,kind=str(c.ComponentIndexType),index=int(c.Index)) for i,k in enumerate(ids) for c in (doc.Objects.FindId(k).GetHighlightedSubObjects() or [])]
            return state
        before=snapshot();trace=[];events=[];marker='Owned SelChain '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        def ended(sender,event):
            if event.CommandEnglishName=='SelChain':events.append(dict(result=str(event.CommandResult),selection=snapshot()))
        R.Commands.Command.EndCommand+=ended
        try:
            if op['steps']:
                from shrink_face_input import drive
                from chain_selection_driver import DispatchTimer
                row=recipe[op['seed']];a,b=[G.Point3d(*p) for p in row['points'][:2]]
                station=G.Point3d((a.X+b.X)*.5,(a.Y+b.Y)*.5,(a.Z+b.Z)*.5)
                if row['kind'] in ('arc','circle'):
                    curve=inputs[op['seed']];station=curve.PointAt(curve.Domain.T0+(curve.Domain.T1-curve.Domain.T0)*.375)
                options=op.get('options',{})
                script='_SelChain _ChainContinuity=_'+options.get('continuity','Tangency')+' _Direction=_'+options.get('direction','Both')+' _GapTolerance='+options.get('gap','0.000001')+' _AngleTolerance='+options.get('angle','0.00005729577951308232')+' _Pause'
                points=[]
                for step in op['steps']:
                    target=step.get('source',op['seed']);other=recipe[target];pts=[G.Point3d(*p) for p in other['points'][:2]]
                    point=G.Point3d((pts[0].X+pts[1].X)*.5,(pts[0].Y+pts[1].Y)*.5,(pts[0].Z+pts[1].Z)*.5)
                    if other['kind'] in ('arc','circle'):
                        curve=inputs[target];point=curve.PointAt(curve.Domain.T0+(curve.Domain.T1-curve.Domain.T0)*.375)
                    points.append([point])
                success=drive(op,points,host,snapshot,trace,'SelChain',script=script,timer_factory=DispatchTimer,accept_last_enter=True,input_ready=lambda: '_Pause' in R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1] and R.Input.RhinoGet.InGetObject(doc))
            else:success=R.RhinoApp.RunScript('_SelChain _Cancel',True)
        finally:R.Commands.Command.EndCommand-=ended
        if R.Commands.Command.InCommand():raise ValueError('SelChain left active input')
        after=snapshot();history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        R.RhinoApp.RunScript('_Redo',False);redo=snapshot();R.RhinoApp.RunScript('_Undo',False);undo=snapshot()
        unchanged=crc==[int(doc.Objects.FindId(k).Geometry.DataCRC(0)) for k in ids]
        if not unchanged:raise ValueError('SelChain edited a source')
        return dict(input=recipe,before=before,trace=trace,events=events,after=after,redo=redo,undo=undo,history=history,success=success,source_unchanged=unchanged),0
    finally:
        R.RhinoApp.RunScript('!',False)
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        for geo in inputs:geo.Dispose()
