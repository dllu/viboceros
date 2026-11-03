"""Owned surface Inset command, document outputs and actual Undo/Redo."""
if __package__:
    from .inset_surface_probe import source
else:
    from inset_surface_probe import source

SCENES = ('rectangle', 'skew', 'concave', 'hole', 'box_one', 'box_pair')


def cases(ready=False):
    return [dict(id=scene + ('_crease' if crease else '_ring') + '_' + mode,
                 op='inset_surface_ready' if ready else 'inset_surface_command', scene=scene, crease_corners=crease,
                 split=mode != 'curves', join_outputs=mode == 'joined')
            for scene in SCENES for crease in (False, True)
            for mode in ('curves', 'pieces', 'joined')]


def validate(op):
    if not isinstance(op, dict):
        raise ValueError('invalid owned surface Inset command recipe')
    if (set(op) != set(('id', 'op', 'scene', 'crease_corners', 'split', 'join_outputs'))
            or op['op'] not in ('inset_surface_command', 'inset_surface_ready') or op['scene'] not in SCENES
            or any(type(op[k]) is not bool for k in ('crease_corners', 'split', 'join_outputs'))
            or (not op['split'] and op['join_outputs'])):
        raise ValueError('invalid owned surface Inset command recipe')
    if op not in cases(op['op'] == 'inset_surface_ready'):
        raise ValueError('surface Inset command differs from its owned case')


def run(op, host, iterations, tolerance):
    validate(op)
    from join_probe import observe_command
    R, S = host['Rhino'], host['System']
    doc = R.RhinoDoc.ActiveDoc
    if iterations != 1 or list(doc.Objects) or R.Commands.Command.InCommand():
        raise ValueError('empty owned idle document required')
    group = layer = -1
    b = source(op['scene'], R)
    previous_tolerance = doc.ModelAbsoluteTolerance
    previous_angle = doc.ModelAngleToleranceRadians
    try:
        doc.ModelAbsoluteTolerance = tolerance['absolute']
        doc.ModelAngleToleranceRadians = tolerance['angular']
        l = R.DocObjects.Layer(); l.Name = 'Inset surface source'
        layer = doc.Layers.Add(l)
        a = R.DocObjects.ObjectAttributes(); a.Name = 'Input'; a.LayerIndex = layer
        a.ObjectColor = S.Drawing.Color.FromArgb(17, 43, 91)
        a.ColorSource = R.DocObjects.ObjectColorSource.ColorFromObject
        oid = doc.Objects.AddBrep(b, a)
        group = doc.Groups.Add('Inset surface group', [oid])
        model = R.FileIO.File3dm()
        try:
            model.Objects.AddBrep(b)
            archive = [int(v) for v in model.ToByteArray()]
        finally: model.Dispose()
        def snapshot():
            rows = []
            for obj in sorted(list(doc.Objects), key=lambda o: o.RuntimeSerialNumber):
                a = obj.Attributes
                row = dict(source=0 if obj.Id == oid else None, name=a.Name,
                           layer=0 if a.LayerIndex == layer else None,
                           color=[int(a.ObjectColor.R), int(a.ObjectColor.G), int(a.ObjectColor.B)],
                           groups=[0 for g in a.GetGroupList() or [] if g == group],
                           selected=bool(obj.IsSelected(False)),
                           selected_faces=[int(c.Index) for c in obj.GetSelectedSubObjects() or []
                                           if c.ComponentIndexType == R.Geometry.ComponentIndexType.BrepFace])
                if isinstance(obj.Geometry, R.Geometry.Brep):
                    row['brep'] = host['_interchange_brep_record'](obj.Geometry)
                elif isinstance(obj.Geometry, R.Geometry.Curve):
                    row['curve'] = host['_interchange_curve_record'](obj.Geometry)
                else: raise ValueError('unexpected Inset output')
                rows.append(row)
            return rows
        doc.ClearUndoRecords(True); doc.ClearRedoRecords()
        if op['scene'].startswith('box'):
            for i in ([0, 1] if op['scene'] == 'box_pair' else [0]):
                doc.Objects.FindId(oid).SelectSubObject(R.Geometry.ComponentIndex(
                    R.Geometry.ComponentIndexType.BrepFace, i), True, True, False)
        else: doc.Objects.Select(oid)
        before = snapshot()
        marker = 'Owned surface Inset ' + str(S.Guid.NewGuid())
        R.RhinoApp.WriteLine(marker)
        yn = lambda value: '_Yes' if value else '_No'
        macro = '_Inset _Loose=' + ('_Off' if op['op'] == 'inset_surface_ready' else '_No')
        macro += ' _IgnoreSeams=_Yes _CreaseCorners=' + yn(op['crease_corners'])
        macro += ' _Split=' + yn(op['split'])
        if op['split']: macro += ' _JoinOutputs=' + yn(op['join_outputs'])
        macro += ' 0.3 _Enter _Cancel _Cancel'
        success, after, events = observe_command(R.Commands.Command, 'Inset',
            lambda: R.RhinoApp.RunScript(macro, True), snapshot, lambda: [], True)
        if R.Commands.Command.InCommand(): raise ValueError('Inset command left active')
        after_script = snapshot()
        history = R.RhinoApp.CommandHistoryWindowText.split(marker, 1)[1].strip()
        R.RhinoApp.RunScript('_Undo', False); undo = snapshot()
        R.RhinoApp.RunScript('_Redo', False); redo = snapshot()
        return dict(archive=archive, before=before, after=after, after_script=after_script,
                    undo=undo, redo=redo, success=success, history=history,
                    results=[e['result'] for e in events if e['name'] == 'Inset']), 0
    finally:
        for obj in list(doc.Objects): doc.Objects.Delete(obj.Id, True)
        if group >= 0: doc.Groups.Delete(group)
        if layer >= 0: doc.Layers.Delete(layer, True)
        doc.ModelAbsoluteTolerance = previous_tolerance
        doc.ModelAngleToleranceRadians = previous_angle
        b.Dispose()
