"""Actual SelUV surface selection and repeated row-picking macros."""
import math
import re

FAMILIES = ('surface', 'surface_closed', 'surface_periodic_u', 'surface_periodic_v', 'surface_nonuniform', 'surface_cubic')


def validate(op):
    allowed = {'op','id','family','artifact_path','actions','grips_on','preselected','selected','cancel','managed','cancel_surface','next_actions'}
    if (not isinstance(op, dict) or set(op) - allowed or op.get('op') != 'select_uv_rows'
            or not isinstance(op.get('id'), str) or re.match(r'^[A-Za-z0-9_-]{1,80}\Z', op['id']) is None
            or op.get('family') not in FAMILIES or not isinstance(op.get('artifact_path'), str)
            or not isinstance(op.get('actions'), list) or len(op['actions']) > 24
            or type(op.get('grips_on', False)) is not bool
            or type(op.get('preselected', False)) is not bool
            or type(op.get('cancel', False)) is not bool
            or type(op.get('managed', False)) is not bool
            or not isinstance(op.get('selected', []), list)
            or any(type(i) is not int or not 0 <= i < 64 for i in op.get('selected', []))):
        raise ValueError('invalid SelUV recipe')
    if type(op.get('cancel_surface', False)) is not bool:
        raise ValueError('invalid SelUV cancellation')
    sequences = [op['actions']] + ([op['next_actions']] if 'next_actions' in op else [])
    if any(not isinstance(actions, list) or len(actions) > 24 for actions in sequences):
        raise ValueError('invalid SelUV continuation')
    for a in [a for actions in sequences for a in actions]:
        if isinstance(a, dict):
            if set(a) == {'parameter'}:
                if (not isinstance(a['parameter'], list) or len(a['parameter']) != 2
                        or any(type(x) not in (int,float) or math.isnan(x) or math.isinf(x) or not 0 <= x <= 1 for x in a['parameter'])):
                    raise ValueError('invalid SelUV surface parameter')
            elif set(a) == {'world'}:
                if (not isinstance(a['world'], list) or len(a['world']) != 3
                        or any(type(x) not in (int, float) or math.isnan(x) or math.isinf(x) for x in a['world'])):
                    raise ValueError('invalid SelUV point')
            elif set(a) != {'grip'} or type(a['grip']) is not int or not 0 <= a['grip'] < 64:
                raise ValueError('invalid SelUV grip')
        elif a not in ('Direction=U', 'Direction=V', 'Direction=Both', 'Toggle', 'HideOthers=Yes', 'HideOthers=No'):
            raise ValueError('invalid SelUV option')


def run(op, host):
    validate(op)
    from grip_navigation_probe import create_source
    from join_probe import observe_command
    R, S = host['Rhino'], host['System']; doc = R.RhinoDoc.ActiveDoc
    if list(doc.Objects) or R.Commands.Command.InCommand():
        raise ValueError('SelUV requires an empty owned document at idle')
    key = peer = S.Guid.Empty; original = None

    def snapshot():
        obj = doc.Objects.FindId(key); other = doc.Objects.FindId(peer)
        if obj is None or not R.Geometry.GeometryBase.GeometryEquals(original, obj.Geometry):
            raise ValueError('SelUV changed source geometry or identity')
        return dict(selected=bool(obj.IsSelected(False)), visible=not bool(obj.IsHidden), grips_on=bool(obj.GripsOn),
            grips=[dict(index=int(g.Index), point=host['_xyz'](g.CurrentLocation), selected=bool(g.IsSelected(False)),
                        hidden=bool(g.IsHidden)) for g in (obj.GetGrips() or [])],
            peer_selected=bool(other.IsSelected(False)), peer_visible=not bool(other.IsHidden))

    try:
        serial = doc.BeginUndoRecord('SelUV sources')
        try:
            if op['family'] in ('surface_nonuniform', 'surface_cubic'):
                cubic = op['family'] == 'surface_cubic'
                order = 4 if cubic else 3
                nu, nv = (4,4) if cubic else (5,4)
                geometry = R.Geometry.NurbsSurface.Create(3,True,order,order,nu,nv)
                for i in range(nu):
                    for j in range(nv):
                        geometry.Points.SetPoint(i,j,host['_point']([float(i),float(j),.15*i*j+.07*i*i]),1.+.15*i*j)
                geometry.KnotsU.CreateUniformKnots(1.); geometry.KnotsV.CreateUniformKnots(1.)
                if not cubic:
                    for i,k in enumerate([0.,0.,.1,.8,1.,1.]): geometry.KnotsU[i] = k
                    for i,k in enumerate([0.,0.,.7,1.,1.]): geometry.KnotsV[i] = k
                if not geometry.IsValid: raise ValueError('invalid SelUV test surface')
            else: geometry = create_source(op['family'], host)
            try: key = doc.Objects.Add(geometry)
            finally: geometry.Dispose()
            peer = doc.Objects.AddPoint(host['_point']([20.,20.,20.]))
            if key == S.Guid.Empty or peer == S.Guid.Empty: raise ValueError('cannot add SelUV sources')
        finally: doc.EndUndoRecord(serial)
        obj = doc.Objects.FindId(key); original = obj.Geometry.Duplicate()
        path = op['artifact_path']
        if path.startswith('/'): path = 'Z:' + path.replace('/', '\\')
        write = R.FileIO.FileWriteOptions()
        write.FileVersion = 8; write.SuppressDialogBoxes = True; write.SuppressAllInput = True
        write.UpdateDocumentPath = False; write.WriteUserData = True
        try:
            if not doc.Write3dmFile(path, write): raise ValueError('cannot save SelUV source and picking view')
        finally: write.Dispose()
        obj.GripsOn = True
        points = [host['_xyz'](g.CurrentLocation) for g in obj.GetGrips()]
        if any(i >= len(points) for i in op.get('selected', [])): raise ValueError('SelUV seed outside grip layout')
        obj.GripsOn = op.get('grips_on', False)
        if obj.GripsOn:
            for g in obj.GetGrips(): g.Select(int(g.Index) in op.get('selected', []))
        serial = doc.BeginUndoRecord('SelUV redo witness')
        try: marker = doc.Objects.AddPoint(host['_point']([30.,30.,30.]))
        finally: doc.EndUndoRecord(serial)
        R.RhinoApp.RunScript('_Undo', False)
        if doc.Objects.FindId(marker) is not None: raise ValueError('cannot prepare SelUV redo witness')
        if op.get('preselected'): doc.Objects.Select(key, True)
        doc.Objects.Select(peer, True)
        def command(actions, cancel=False, cancel_surface=False):
            words = ['_SelUV']
            if not cancel_surface: words += ['_SelID', str(key)]
            for a in actions:
                if isinstance(a, dict):
                    if 'parameter' in a:
                        surface = doc.Objects.FindId(key).Geometry.Surfaces[0]
                        point = host['_xyz'](surface.PointAt(*a['parameter']))
                    elif 'world' in a: point = a['world']
                    else:
                        if a['grip'] >= len(points): raise ValueError('SelUV point outside grip layout')
                        point = points[a['grip']]
                    words.append('w' + host['_command_point'](point))
                elif '=' in a:
                    name, value = a.split('='); words.append('_' + name + '=_' + value)
                else: words.append('_' + a)
            words.append('_Cancel' if cancel or cancel_surface else '_Enter')
            macro = ' '.join(words); history = R.RhinoApp.CommandHistoryWindowText
            initial = snapshot()
            host['_record_progress'](op['id'] + ' ' + macro)
            traces = []
            def execute():
                if not op.get('managed'): return R.RhinoApp.RunScript(macro, False)
                from sel_uv_input import drive
                returned, trace = drive(host, doc.Objects.FindId(key), doc.Objects.FindId(peer), words[1:], len(points))
                traces.append(trace)
                return returned
            success, after, events = observe_command(R.Commands.Command, 'SelUV', execute, snapshot, lambda: [], True)
            value = dict(macro=macro, history=R.RhinoApp.CommandHistoryWindowText[len(history):],
                before=initial, success=success, after=after, after_script=snapshot(), events=events)
            if traces: value['input_trace'] = traces[0]
            return value
        result = command(op['actions'], op.get('cancel', False), op.get('cancel_surface', False))
        again = command(op['next_actions']) if 'next_actions' in op else None
        R.RhinoApp.RunScript('_Redo', False)
        value = dict(result, recipe=op, source_id=str(key), peer_id=str(peer), points=points,
            redo_retained=doc.Objects.FindId(marker) is not None)
        if again is not None: value['again'] = again
        import json
        with open(path + '.capture.json', 'w') as stream: json.dump(value, stream, indent=2, allow_nan=False)
        return value, 0
    finally:
        if original is not None: original.Dispose()
        for obj in list(doc.Objects):
            if obj.GripsOn: obj.GripsOn = False
            doc.Objects.Delete(obj.Id, True)
