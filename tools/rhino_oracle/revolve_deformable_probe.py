"""Observe the public Revolve command in an owned private native document."""
if __package__:
    from .revolve_deformable_cases import validate
else:
    from revolve_deformable_cases import validate


def run(op, host, iterations, tolerance):
    validate(op)
    from join_probe import observe_command
    R, S = host['Rhino'], host['System']
    G = R.Geometry
    doc = R.RhinoDoc.ActiveDoc
    if iterations != 1 or list(doc.Objects) or R.Commands.Command.InCommand():
        raise ValueError('empty owned idle document required')
    source = S.Guid.Empty
    def snapshot():
        rows = []
        for obj in sorted(list(doc.Objects), key=lambda o: int(o.RuntimeSerialNumber)):
            g = obj.Geometry
            row = dict(source=obj.Id == source, selected=int(obj.IsSelected(False)))
            if isinstance(g, G.Curve):
                row.update(kind='curve', curve=host['_nurbs_curve_definition'](g))
            elif isinstance(g, G.Brep):
                row.update(kind='surface', surfaces=[host['_nurbs_surface_definition'](f.UnderlyingSurface()) for f in g.Faces])
            elif isinstance(g, G.Surface):
                row.update(kind='surface', surfaces=[host['_nurbs_surface_definition'](g)])
            else:
                raise ValueError('unexpected revolve output: ' + str(g.GetType().Name))
            rows.append(row)
        return rows
    try:
        curve = host['_nurbs_curve_from_definition'](op['curve'])
        try: source = doc.Objects.AddCurve(curve)
        finally: curve.Dispose()
        if source == S.Guid.Empty: raise ValueError('source admission failed')
        doc.Objects.Select(source)
        doc.ClearUndoRecords(True)
        before = snapshot()
        marker = 'Owned deformable revolution ' + str(S.Guid.NewGuid())
        R.RhinoApp.WriteLine(marker)
        point = lambda p: 'w' + ','.join('%.17g' % v for v in p)
        macro = ('_-Revolve %s %s _Output=_Surface _Deformable=_Yes _PointCount=%d '
            '_SplitAtTangents=_No _DeleteInput=_%s _AskForStartAngle=_Yes %.17g %.17g') % (
                point(op['origin']), point(op['axis_end']), op['point_count'],
                'Yes' if op['delete_input'] else 'No', op['start_degrees'], op['sweep_degrees'])
        success, created, events = observe_command(R.Commands.Command, 'Revolve',
            lambda: R.RhinoApp.RunScript(macro, True), snapshot, lambda: [], True)
        after = snapshot()
        history = R.RhinoApp.CommandHistoryWindowText.split(marker, 1)[1].strip()
        if op.get('cancel_before_history', True): R.RhinoApp.RunScript('_Cancel _Cancel', False)
        R.RhinoApp.RunScript('_Undo', False)
        undo = snapshot()
        R.RhinoApp.RunScript('_Redo', False)
        redo = snapshot()
        return dict(success=success, before=before, created=created, after=after,
            undo=undo, redo=redo, events=events, history=history, macro=macro), 0
    finally:
        R.RhinoApp.RunScript('!', False)
        for obj in list(doc.Objects): doc.Objects.Delete(obj.Id, True)
        doc.ClearUndoRecords(True)
