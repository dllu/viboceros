"""Owned surface-to-point commands, including source retention and history."""
import copy


def request():
    try:
        from .surface_apex_extrusion_probe import request as sdk_request
    except (ImportError, ValueError, SystemError):
        from surface_apex_extrusion_probe import request as sdk_request
    q = sdk_request()
    operations = []
    for recipe in q['operations']:
        for solid in [False, True]:
            for delete in [False, True]:
                for preselect in [False, True]:
                    op = copy.deepcopy(recipe)
                    op.update(op='surface_apex_command', solid=solid, delete_input=delete,
                              preselect=preselect, id=recipe['id']+'-{}{}{}'.format(int(solid), int(delete), int(preselect)))
                    operations.append(op)
    q['operations'] = operations
    return q


def validate(op):
    try:
        from .surface_apex_extrusion_probe import validate as sdk_validate
    except (ImportError, ValueError, SystemError):
        from surface_apex_extrusion_probe import validate as sdk_validate
    if (not isinstance(op, dict) or op.get('op') != 'surface_apex_command'
            or type(op.get('delete_input')) is not bool or type(op.get('preselect')) is not bool):
        raise ValueError('invalid surface apex command options')
    recipe = {k: v for k, v in op.items() if k not in ('delete_input', 'preselect')}
    recipe['op'] = 'surface_apex_extrusion'
    sdk_validate(recipe)


def run(op, host, iterations, tolerance):
    validate(op)
    try:
        from .join_probe import observe_command
    except (ImportError, ValueError, SystemError):
        from join_probe import observe_command
    R, S = host['Rhino'], host['System']
    G, doc = R.Geometry, R.RhinoDoc.ActiveDoc
    if iterations != 1 or list(doc.Objects) or doc.UndoRecordingIsActive:
        raise ValueError('empty owned idle document required')
    saved_tolerance = (doc.ModelAbsoluteTolerance, doc.ModelRelativeTolerance, doc.ModelAngleToleranceRadians)
    source = host['_object_source'](op['source'], tolerance)
    brep, cap, source_id = None, None, None
    def snapshot():
        outputs = []
        for obj in sorted(list(doc.Objects), key=lambda o: o.RuntimeSerialNumber):
            if obj.Id == source_id:
                continue
            b = obj.Geometry
            if not isinstance(b, G.Brep) or not b.IsValid:
                raise ValueError('invalid surface apex command product')
            mass = None
            try:
                volume = None
                if b.IsSolid:
                    mass = G.VolumeMassProperties.Compute(b)
                    if mass is None:
                        raise ValueError('surface apex volume failed')
                    volume = abs(float(mass.Volume))
                bounds = b.GetBoundingBox(True)
                outputs.append(dict(is_solid=bool(b.IsSolid), face_count=int(b.Faces.Count), volume=volume,
                                    bounds=[host['_xyz'](bounds.Min), host['_xyz'](bounds.Max)]))
            finally:
                if mass is not None:
                    mass.Dispose()
        original = doc.Objects.FindId(source_id) if source_id is not None else None
        return dict(source_present=original is not None,
                    source_unchanged=original is None or int(original.Geometry.DataCRC(0)) == source_crc,
                    breps=outputs)
    try:
        doc.ModelAbsoluteTolerance = tolerance['absolute']
        doc.ModelRelativeTolerance = tolerance['relative']
        doc.ModelAngleToleranceRadians = tolerance['angular']
        if isinstance(source, G.Surface):
            brep = source.ToBrep()
        elif isinstance(source, G.Brep):
            brep = source.DuplicateBrep()
        if brep is None or op['face'] >= brep.Faces.Count:
            raise ValueError('invalid surface apex face')
        cap = brep.Faces[op['face']].DuplicateFace(False)
        if cap is None or not cap.IsValid:
            raise ValueError('invalid surface apex source face')
        record = doc.BeginUndoRecord('Owned surface apex source')
        try:
            source_id = doc.Objects.AddBrep(cap)
            if source_id == S.Guid.Empty:
                raise ValueError('surface apex source insertion failed')
            if op['preselect']:
                doc.Objects.Select(source_id)
        finally:
            doc.EndUndoRecord(record)
        source_crc = int(doc.Objects.FindId(source_id).Geometry.DataCRC(0))
        doc.ClearUndoRecords(True)
        marker = 'Owned surface apex '+str(S.Guid.NewGuid())
        R.RhinoApp.WriteLine(marker)
        select = '' if op['preselect'] else ' _SelID '+str(source_id)+' _Enter'
        macro = '_ExtrudeSrfToPoint'+select+' _Solid=_'+('Yes' if op['solid'] else 'No')
        macro += ' _DeleteInput=_'+('Yes' if op['delete_input'] else 'No')
        macro += ' w'+host['_command_point'](op['apex'])+' _Cancel _Cancel _Cancel'
        success, after, events = observe_command(R.Commands.Command, 'ExtrudeSrfToPoint',
            lambda: R.RhinoApp.RunScript(macro, True), snapshot, lambda: [], True)
        history = R.RhinoApp.CommandHistoryWindowText.split(marker, 1)[1].strip()
        if not success or 'Unknown command:' in history or not after['breps']:
            raise ValueError('surface apex command failed: '+history)
        R.RhinoApp.RunScript('_Undo', False)
        undo = snapshot()
        R.RhinoApp.RunScript('_Redo', False)
        redo = snapshot()
        return dict(after=after, undo=undo, redo=redo, history=history, events=events,
                    document_tolerance=dict(absolute=float(doc.ModelAbsoluteTolerance),
                        relative=float(doc.ModelRelativeTolerance), angular=float(doc.ModelAngleToleranceRadians))), 0
    finally:
        errors = []
        def cleanup(action):
            try:
                action()
            except Exception as error:
                errors.append(str(error))
        cleanup(lambda: R.RhinoApp.RunScript('!', False))
        objects = []
        cleanup(lambda: objects.extend(list(doc.Objects)))
        for obj in objects:
            cleanup(lambda obj=obj: doc.Objects.Delete(obj.Id, True))
        for geometry in [cap, brep, source]:
            if geometry is not None:
                cleanup(geometry.Dispose)
        for name, value in zip(['ModelAbsoluteTolerance', 'ModelRelativeTolerance', 'ModelAngleToleranceRadians'], saved_tolerance):
            cleanup(lambda name=name, value=value: setattr(doc, name, value))
        if errors:
            raise ValueError('surface apex cleanup failed: '+str(errors))
