"""Observe directional selection commands on public, owned object grips."""
import re

COMMANDS = ('NextU', 'PrevU', 'NextV', 'PrevV', 'AddNextU', 'AddPrevU',
            'AddNextV', 'AddPrevV', 'SelU', 'SelV')
KINDS = ('curve', 'rational', 'closed', 'periodic', 'line', 'polyline', 'circle',
         'arc', 'surface', 'surface_closed', 'mesh', 'polycurve',
         'surface_periodic_u', 'surface_periodic_v')


def validate(op):
    if (not isinstance(op, dict) or set(op) != {'op', 'id', 'artifact_path', 'sources', 'actions'} or op.get('op') != 'grip_navigation'
            or not isinstance(op.get('id'), str)
            or re.match(r'^[A-Za-z0-9_-]{1,80}\Z', op['id']) is None
            or not isinstance(op.get('artifact_path'), str)
            or not isinstance(op.get('sources'), list) or not 1 <= len(op['sources']) <= 4
            or not isinstance(op.get('actions'), list) or not 1 <= len(op['actions']) <= 96):
        raise ValueError('invalid bounded grip navigation recipe')
    for source in op['sources']:
        if (not isinstance(source, dict) or set(source) != {'kind', 'mode', 'parent'} or source['kind'] not in KINDS
                or source['mode'] not in ('control', 'edit') or type(source['parent']) is not bool
                or (source['mode'] == 'edit' and (source['kind'].startswith('surface') or source['kind'] == 'mesh'))):
            raise ValueError('invalid grip navigation source')
    for action in op['actions']:
        if (not isinstance(action, dict) or set(action) - {'command', 'seed'}
                or action.get('command') not in COMMANDS
                or ('seed' in action and action['seed'] not in ('first', 'last', 'middle', 'ends', 'all', 'empty'))):
            raise ValueError('invalid grip navigation action')


def create_source(kind, host):
    from grip_transform_probe import create_source as basic_source
    R = host['Rhino']
    if kind == 'polycurve':
        c = R.Geometry.PolyCurve()
        a = basic_source('line', host)
        b = R.Geometry.LineCurve(a.PointAtEnd, host['_point']([3., 4., 2.]))
        try:
            if not c.Append(a) or not c.Append(b): raise ValueError('cannot build navigation polycurve')
        finally:
            a.Dispose(); b.Dispose()
        return c
    if not kind.startswith('surface_periodic_'):
        return basic_source(kind, host)
    axis = 0 if kind.endswith('_u') else 1
    rail = basic_source('periodic', host)
    try:
        n = rail.Points.Count
        surface = R.Geometry.NurbsSurface.Create(3, False, 3, 3, n if axis == 0 else 3, 3 if axis == 0 else n)
        for i in range(n):
            p = rail.Points[i].Location
            for j in range(3):
                surface.Points.SetPoint(i if axis == 0 else j, j if axis == 0 else i,
                    R.Geometry.Point3d(p.X, p.Y, float(j)))
        (surface.KnotsU if axis == 0 else surface.KnotsV).CreatePeriodicKnots(1.)
        (surface.KnotsV if axis == 0 else surface.KnotsU).CreateUniformKnots(1.)
        if not surface.IsValid: raise ValueError('invalid periodic navigation surface')
        return surface
    finally:
        rail.Dispose()


def seed_indices(seed, count):
    return dict(first=[0], last=[count - 1], middle=[count // 2],
                ends=sorted(set([0, count - 1])), all=list(range(count)), empty=[])[seed]


def run(op, host):
    validate(op)
    from join_probe import observe_command
    R, S = host['Rhino'], host['System']; doc = R.RhinoDoc.ActiveDoc
    if list(doc.Objects) or R.Commands.Command.InCommand():
        raise ValueError('grip navigation requires an empty owned document at idle')
    ids, originals = [], []
    peer = S.Guid.Empty

    def snapshot():
        rows = []
        for key, original in zip(ids, originals):
            obj = doc.Objects.FindId(key)
            if obj is None or not R.Geometry.GeometryBase.GeometryEquals(original, obj.Geometry):
                raise ValueError('grip navigation changed geometry or identity')
            rows.append(dict(selected=bool(obj.IsSelected(False)),
                grips=[dict(index=int(g.Index), point=host['_xyz'](g.CurrentLocation), selected=bool(g.IsSelected(False)))
                       for g in (obj.GetGrips() or [])]))
        return dict(sources=rows, peer_selected=bool(doc.Objects.FindId(peer).IsSelected(False)))

    try:
        serial = doc.BeginUndoRecord('Navigation sources')
        try:
            for i, source in enumerate(op['sources']):
                geometry = create_source(source['kind'], host)
                try:
                    attributes = R.DocObjects.ObjectAttributes(); attributes.Name = 'navigation-' + str(i)
                    key = doc.Objects.Add(geometry, attributes)
                    if key == S.Guid.Empty: raise ValueError('cannot add navigation source')
                    ids.append(key); originals.append(doc.Objects.FindId(key).Geometry.Duplicate())
                finally: geometry.Dispose()
            peer = doc.Objects.AddPoint(host['_point']([20., 20., 20.]))
            if peer == S.Guid.Empty: raise ValueError('cannot add navigation peer')
        finally: doc.EndUndoRecord(serial)
        model = R.FileIO.File3dm()
        try:
            for key in ids:
                obj = doc.Objects.FindId(key); model.Objects.Add(obj.Geometry, obj.Attributes)
            path = op['artifact_path']
            if path.startswith('/'): path = 'Z:' + path.replace('/', '\\')
            if not model.Write(path, 8): raise ValueError('cannot save navigation sources')
        finally: model.Dispose()
        for key, source in zip(ids, op['sources']):
            obj = doc.Objects.FindId(key)
            if source['mode'] == 'edit':
                doc.Objects.Select(key, True)
                if not R.RhinoApp.RunScript('_EditPtOn', False): raise ValueError('cannot display edit points')
            else: obj.GripsOn = True
            if not obj.GetGrips(): raise ValueError('navigation source has no grips')
        # Leave a known redo item available before the first selector.
        serial = doc.BeginUndoRecord('Navigation redo witness')
        try: marker = doc.Objects.AddPoint(host['_point']([30., 30., 30.]))
        finally: doc.EndUndoRecord(serial)
        R.RhinoApp.RunScript('_Undo', False)
        if doc.Objects.FindId(marker) is not None: raise ValueError('cannot prepare navigation redo witness')
        for key, source in zip(ids, op['sources']):
            if source['parent']: doc.Objects.Select(key, True)
        doc.Objects.Select(peer, True)
        before = snapshot(); steps = []
        for action in op['actions']:
            if 'seed' in action:
                for key in ids:
                    grips = doc.Objects.FindId(key).GetGrips()
                    picks = seed_indices(action['seed'], len(grips))
                    for grip in grips: grip.Select(int(grip.Index) in picks)
            initial = snapshot(); command = action['command']
            host['_record_progress'](op['id'] + ' ' + command)
            macro = '_' + command
            _, value, events = observe_command(R.Commands.Command, command,
                lambda: R.RhinoApp.RunScript(macro, False), snapshot, lambda: [], True)
            result = [e['result'] for e in events if e['name'] == command]
            if len(result) != 1: raise ValueError('missing navigation command result')
            steps.append(dict(command=command, macro=macro, before=initial, after=value, result=result[0]))
        R.RhinoApp.RunScript('_Redo', False)
        redo_retained = doc.Objects.FindId(marker) is not None
        value = dict(before=before, steps=steps, redo_retained=redo_retained)
        # Completed per-case diagnostics survive a later case failing. The
        # generator publishes a corpus only after every requested case returns.
        import json
        with open(path + '.capture.json', 'w') as stream:
            json.dump(value, stream, indent=2, allow_nan=False)
        return value, 0
    finally:
        for geometry in originals: geometry.Dispose()
        for obj in list(doc.Objects):
            if obj.GripsOn: obj.GripsOn = False
            doc.Objects.Delete(obj.Id, True)
