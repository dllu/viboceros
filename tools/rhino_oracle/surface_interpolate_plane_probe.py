"""Owned public InfinitePlane input for InterpCrvOnSrf."""
from copy import deepcopy
if __package__:
    from .surface_interpolate_plane_cases import CASES, request, managed_request, bounded_request, workflow_request, audit_request, validate, metadata
else:
    from surface_interpolate_plane_cases import CASES, request, managed_request, bounded_request, workflow_request, audit_request, validate, metadata

def run(op, host, iterations, tolerance):
    validate(op)
    from join_probe import observe_command
    R, S = host['Rhino'], host['System']
    G = R.Geometry
    doc = R.RhinoDoc.ActiveDoc
    if iterations != 1 or list(doc.Objects) or R.Commands.Command.InCommand():
        raise ValueError('empty owned idle document required')
    viewport = doc.Views.ActiveView.ActiveViewport
    old = (doc.Layers.CurrentLayerIndex, doc.ModelAbsoluteTolerance, doc.ModelRelativeTolerance, doc.ModelAngleToleranceRadians, viewport.ConstructionPlane())
    layers = []
    source = None
    reference = None
    group = None
    named = None

    def snapshot():
        rows = []
        for o in sorted(list(doc.Objects), key=lambda o: int(o.RuntimeSerialNumber)):
            a = o.Attributes
            rows.append(dict(role='source' if o.Id == source else 'reference' if o.Id==reference else 'output', selected=int(o.IsSelected(False)),
                             attributes=dict(name=a.Name or '', layer=doc.Layers[a.LayerIndex].Name,
                                             color_source=str(a.ColorSource), color=[int(a.ObjectColor.R), int(a.ObjectColor.G), int(a.ObjectColor.B)],
                                             groups=[doc.Groups.GroupName(i) for i in (a.GetGroupList() or [])]),
                             curve=host['_nurbs_curve_definition'](o.Geometry), closed=bool(o.Geometry.IsClosed), native_valid=bool(o.Geometry.IsValid)))
        return dict(objects=rows, group_alive=group is not None and not doc.Groups[group].IsDeleted)

    try:
        doc.ModelAbsoluteTolerance = tolerance['absolute']
        doc.ModelRelativeTolerance = tolerance['relative']
        doc.ModelAngleToleranceRadians = tolerance['angular']
        viewport.SetConstructionPlane(G.Plane.WorldXY)
        for name in ['Interpolation plane source', 'Interpolation plane output']:
            layer = R.DocObjects.Layer(); layer.Name = name
            layers.append(doc.Layers.Add(layer)); layer.Dispose()
        curve = G.PolylineCurve([G.Point3d(*p) for p in [[-20.,-20.,-20.],[20.,-20.,20.],[20.,20.,-20.],[-20.,20.,20.],[-20.,-20.,-20.]]]) if 'bounded' in op['case'] else G.LineCurve(G.Point3d(50., 60., 70.), G.Point3d(60., 60., 70.))
        a = R.DocObjects.ObjectAttributes(); a.LayerIndex = layers[0]; a.Name = 'sentinel'
        source = None if op['case'].endswith('_audit_empty') else doc.Objects.AddCurve(curve, a); a.Dispose(); curve.Dispose()
        if op['case'].endswith(('_object','_around')):
            curve=G.LineCurve(G.Point3d(-10.,0.,4.),G.Point3d(10.,0.,4.)) if op['case'].endswith('_around') else G.PolylineCurve([G.Point3d(*p) for p in [[-10.,-10.,4.],[10.,-10.,4.],[10.,10.,4.],[-10.,10.,4.],[-10.,-10.,4.]]])
            a=R.DocObjects.ObjectAttributes();a.LayerIndex=layers[0];a.Name='plane-reference'
            reference=doc.Objects.AddCurve(curve,a);a.Dispose();curve.Dispose()
        if op['case'].endswith('_named'):
            named=doc.NamedConstructionPlanes.Add('InterpolationPlane',G.Plane(G.Point3d(0.,0.,6.),G.Vector3d.ZAxis))
        if op['case'].endswith('_cplane'):
            viewport.SetConstructionPlane(G.Plane(G.Point3d(2.,3.,4.),G.Vector3d.XAxis))
        group = None if source is None else doc.Groups.Add('Interpolation plane sources', [source]+([reference] if reference is not None else []))
        doc.Layers.SetCurrentLayerIndex(layers[1], True)
        doc.ClearUndoRecords(True)
        before = snapshot()
        plane, points, finish = deepcopy(CASES[op['case']])
        if reference is not None:plane=plane.replace('@reference',str(reference))
        def word(p):
            if isinstance(p,dict):return 'r'+','.join(format(x,'.17g') for x in p['relative'])
            return p if isinstance(p, str) else 'w' + ','.join(format(x, '.17g') for x in p)
        macro = '_-InterpCrvOnSrf _IP ' + plane + ' ' + ' '.join(word(p) for p in points) + ' ' + finish + ' _Cancel _Cancel _Cancel'
        marker = 'Owned plane interpolation ' + str(S.Guid.NewGuid())
        R.RhinoApp.WriteLine(marker)
        traces=[]
        def invoke():
            if not op['case'].startswith('managed_'):return R.RhinoApp.RunScript(macro,True)
            from conic_getter_input import drive
            actions=[p.lstrip('_') if isinstance(p,str) else p for p in points+[finish]]
            returned,trace=drive(dict(op='surface_interpolate_plane',id=op['id'],actions=actions),host,[],
                                 'InterpCrvOnSrf',start_script='_InterpCrvOnSrf _IP '+plane+' _Pause')
            traces.append(trace);return returned
        from surface_interpolate_plane_input import guard
        with guard(host,op['case'],op['id']) as (receipts,errors):
            success, created, events = observe_command(R.Commands.Command, 'InterpCrvOnSrf', invoke, snapshot, lambda: [], True)
        if errors and '_audit_' not in op['case']:raise ValueError(errors[0])
        after = snapshot()
        history = R.RhinoApp.CommandHistoryWindowText.split(marker, 1)[1].strip()
        if R.Commands.Command.InCommand():
            raise ValueError('interpolation did not return to idle: ' + history)
        if success:
            R.RhinoApp.RunScript('_Undo', False); undo = snapshot()
            R.RhinoApp.RunScript('_Redo', False); redo = snapshot()
        else:
            undo, redo = before, after
        result=dict(success=success, before=before, after=after, undo=undo, redo=redo,
                    macro=macro, history=history, events=events, created=created,input_trace=traces,cancel_guard=receipts,returned_idle=True,input_errors=errors)
        result.update(metadata(op['case']))
        return result, 0
    finally:
        R.RhinoApp.RunScript('!', False)
        for o in list(doc.Objects): doc.Objects.Delete(o.Id, True)
        doc.ClearUndoRecords(True)
        if group is not None: doc.Groups.Delete(group)
        if named is not None:doc.NamedConstructionPlanes.Delete(named)
        doc.Layers.SetCurrentLayerIndex(old[0], True)
        doc.ModelAbsoluteTolerance, doc.ModelRelativeTolerance, doc.ModelAngleToleranceRadians = old[1:4]
        viewport.SetConstructionPlane(old[4])
        for index in reversed(layers): doc.Layers.Delete(index, True)
