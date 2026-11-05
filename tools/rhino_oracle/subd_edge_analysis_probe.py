"""Public SubD edge curves plus actual owned ShowEdges/ZoomNaked workflows."""
if __package__:
    from .subd_edge_analysis_cases import validate
else:
    from subd_edge_analysis_cases import validate


def source(scene, host):
    if scene.startswith('asset_'):
        import os
        path = os.path.join(os.path.dirname(__file__), 'fixtures', 'subd_soft_nurbs_inputs', scene[6:] + '.3dm')
        model = host['Rhino'].FileIO.File3dm.Read(path)
        try:
            rows = [obj.Geometry for obj in model.Objects if isinstance(obj.Geometry, host['Rhino'].Geometry.SubD)]
            if len(rows) != 1:
                raise ValueError('owned soft-crease source missing')
            return rows[0].Duplicate()
        finally:
            model.Dispose()
    from subd_direction_cases import source as recipe
    from subd_to_nurbs_probe import make_source
    return make_source(recipe(scene), host)


def edges(subd, host):
    rows = []
    for index, edge in enumerate(subd.Edges):
        curve = edge.ToNurbsCurve(False)
        try:
            rows.append(dict(index=index, id=int(edge.Id), tag=int(edge.Tag), faces=int(edge.FaceCount),
                             control=[host['_xyz'](edge.VertexFrom.ControlNetPoint), host['_xyz'](edge.VertexTo.ControlNetPoint)],
                             ends=[host['_xyz'](edge.VertexFrom.SurfacePoint()), host['_xyz'](edge.VertexTo.SurfacePoint())],
                             curve=None if curve is None else dict(domain=[float(curve.Domain.T0), float(curve.Domain.T1)],
                                points=[host['_xyz'](curve.PointAt(curve.Domain.ParameterAt(i / 32.))) for i in range(33)])))
        finally:
            if curve is not None:
                curve.Dispose()
    return rows


def run(op, host, iterations, tolerance):
    validate(op)
    from join_probe import observe_command
    from owned_panel import inspect
    from subd_direction_probe import geometry
    R, S = host['Rhino'], host['System']
    G, doc = R.Geometry, R.RhinoDoc.ActiveDoc
    if iterations != 1 or list(doc.Objects) or R.Commands.Command.InCommand():
        raise ValueError('empty owned idle document required')
    subd = source(op['scene'], host)
    old = doc.SubDAppearance
    finish_input = None
    try:
        key = doc.Objects.AddSubD(subd)
        if key == S.Guid.Empty:
            raise ValueError('cannot add edge-analysis source')
        doc.SubDAppearance = G.SubDComponentLocation.ControlNet if op['flat'] else G.SubDComponentLocation.Surface
        sdk = edges(doc.Objects.FindId(key).Geometry, host)
        model = R.FileIO.File3dm()
        try:
            model.Objects.AddSubD(doc.Objects.FindId(key).Geometry)
            source_archive = S.Convert.ToBase64String(model.ToByteArray())
        finally:
            model.Dispose()

        def snapshot():
            obj = doc.Objects.FindId(key)
            return dict(geometry=geometry(obj.Geometry, host), selected=bool(obj.IsSelected(False)),
                        objects=len(list(doc.Objects)), points=[host['_xyz'](o.Geometry.Location) for o in sorted(doc.Objects, key=lambda o: int(o.RuntimeSerialNumber)) if isinstance(o.Geometry, G.Point)])

        R.RhinoApp.RunScript('_ShowEdgesOff', False)
        doc.Objects.UnselectAll()
        doc.ClearUndoRecords(True)
        R.RhinoApp.RunScript('_Point w40,50,60', False)
        R.RhinoApp.RunScript('_Undo', False)
        if op['preselect']:
            doc.Objects.Select(key)
        before = snapshot()
        marker = 'Owned SubD edges ' + str(S.Guid.NewGuid())
        R.RhinoApp.WriteLine(marker)
        select = '' if op['preselect'] else ' _SelID ' + str(key) + ' _Enter'
        macro = '_' + op['command'] + select
        if op['command'] != 'ShowEdges':
            macro += ' ' + ' '.join('_' + a for a in op['actions']) + ' _'+op.get('finish','Enter')
        success, event_state, events = observe_command(R.Commands.Command, op['command'], lambda: R.RhinoApp.RunScript(macro, True), snapshot, lambda: [], True)
        R.RhinoApp.Wait()
        states = []
        if op['command'] == 'ShowEdges':
            for action in op['actions']:
                label = dict(All='All edges', Naked='Naked edges', NonManifold='Non-manifold edges')[action.split('=')[1]]
                inspect('Edge Analysis', label, op.get('native_click',False))
                R.RhinoApp.Wait()
                states.append(dict(action=action, controls=inspect('Edge Analysis')))
        if op.get('zoom'):
            from edge_zoom_input import start
            finish_input = start(R,S,['_'+token for token in op['zoom']]+[''])
            inspect('Edge Analysis','Zoom',True)
            R.RhinoApp.Wait()
            try:input_state=finish_input()
            except ValueError as error:input_state=dict(error=str(error),prompt=str(R.RhinoApp.CommandPrompt))
            states.append(dict(action='Zoom',input=input_state,controls=inspect('Edge Analysis')))
        after = snapshot()
        changed = bool(after['points'])
        if changed:
            R.RhinoApp.RunScript('_Undo', False)
            undo = snapshot()
            R.RhinoApp.RunScript('_Redo', False)
            redo = snapshot()
        else:
            R.RhinoApp.RunScript('_Redo', False)
            redo = snapshot()
            R.RhinoApp.RunScript('_Undo', False)
            undo = snapshot()
        return dict(sdk=sdk, source_archive=source_archive, before=before, event_state=event_state, after=after, undo=undo, redo=redo,
                    success=success, events=events, modes=states, macro=macro,
                    history=R.RhinoApp.CommandHistoryWindowText.split(marker, 1)[1].strip()), 0
    finally:
        if finish_input is not None:finish_input.stop()
        R.RhinoApp.RunScript('!', False)
        R.RhinoApp.RunScript('_ShowEdgesOff', False)
        for obj in list(doc.Objects):
            doc.Objects.Delete(obj.Id, True)
        doc.SubDAppearance = old
        subd.Dispose()
