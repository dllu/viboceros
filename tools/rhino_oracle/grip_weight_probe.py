"""Full public Weight/Undo/Redo states over shared curve and surface grip models."""
import math
import re
if __package__:
    from .delete_grips_probe import KINDS, SEEDS, create_source, picks
else:
    from delete_grips_probe import KINDS, SEEDS, create_source, picks


def validate(op):
    if (not isinstance(op, dict) or set(op) != {'op','id','artifact_path','sources','peer','off','dash','inputs','pick','repeat'}
            or op.get('op') != 'grip_weight' or not isinstance(op.get('id'), str)
            or re.match(r'^[A-Za-z0-9_-]{1,80}\Z', op['id']) is None
            or not isinstance(op.get('artifact_path'), str)
            or not isinstance(op.get('sources'), list) or not 1 <= len(op['sources']) <= 4
            or any(type(op.get(k)) is not bool for k in ('peer','off','dash','pick','repeat'))
            or not isinstance(op.get('inputs'), list) or not 1 <= len(op['inputs']) <= 4):
        raise ValueError('invalid Weight recipe')
    for value in op['inputs']:
        if type(value) in (int,float):
            if (math.isnan(value) or math.isinf(value)) or abs(value)>1e6: raise ValueError('invalid Weight number')
        elif value not in ('Enter','Cancel'): raise ValueError('invalid Weight input')
    if not op['dash'] and (len(op['inputs'])>2 or len(op['inputs'])==2 and (type(op['inputs'][0]) not in (int,float) or op['inputs'][1]!='Cancel')):
        raise ValueError('invalid Weight dialog input')
    for s in op['sources']:
        if (not isinstance(s,dict) or set(s) != {'kind','mode','selected','hidden','parent'}
                or s['kind'] not in KINDS or s['mode'] not in ('control','edit')
                or s['selected'] not in SEEDS or s['hidden'] not in ('empty','first','middle','all')
                or type(s['parent']) is not bool
                or s['mode']=='edit' and s['kind'].startswith(('surface','mesh'))
                or s['selected'] in ('row_u','row_v','cross_rows') and not s['kind'].startswith('surface')
                or s['selected'] in ('neighbor_hidden','next_hidden') and (s['kind'] not in ('curve','rational','periodic') or s['mode']!='control' or s['hidden']!='middle')):
            raise ValueError('invalid Weight source')


def run(op, host):
    validate(op)
    from join_probe import observe_command
    from weight_dialog import watch
    R, S = host['Rhino'], host['System']; doc = R.RhinoDoc.ActiveDoc
    if list(doc.Objects) or R.Commands.Command.InCommand(): raise ValueError('Weight capture needs an empty owned idle document')
    ids = []; group = -1; peer = marker = S.Guid.Empty
    input_log = []
    names = ['source' + str(i) for i in range(len(op['sources']))] + ['peer']

    def snapshot():
        rows = []; seen = set(); handles = ids + [peer]
        for obj in sorted(list(doc.Objects), key=lambda o: o.RuntimeSerialNumber):
            if obj.Id == marker: continue
            name = obj.Attributes.Name
            if name not in names or name in seen: raise ValueError('unexpected Weight output identity')
            seen.add(name); index = names.index(name); g = obj.Geometry
            memberships = list(obj.Attributes.GetGroupList() or [])
            if any(i != group for i in memberships): raise ValueError('unexpected Weight group')
            row = dict(role=name, same_id=obj.Id == handles[index], name=name,
                color=[int(obj.Attributes.ObjectColor.R), int(obj.Attributes.ObjectColor.G), int(obj.Attributes.ObjectColor.B)],
                grouped=group in memberships, selected=bool(obj.IsSelected(False)), visible=not bool(obj.IsHidden),
                grips_on=bool(obj.GripsOn), kind=g.GetType().Name,
                grips=[dict(index=int(p.Index), point=host['_xyz'](p.CurrentLocation), selected=bool(p.IsSelected(False)),
                    hidden=bool(p.IsHidden), selectable=bool(p.IsSelectable(True,False,False,False)), weight=float(p.Weight)) for p in (obj.GetGrips() or [])])
            if isinstance(g, R.Geometry.Curve): row['curve'] = host['_nurbs_curve_definition'](g)
            if isinstance(g, R.Geometry.PolyCurve):
                segments = list(g.DuplicateSegments())
                try: row['segments'] = [dict(kind=s.GetType().Name, curve=host['_nurbs_curve_definition'](s)) for s in segments]
                finally:
                    for segment in segments: segment.Dispose()
            elif isinstance(g, R.Geometry.Curve): pass
            elif isinstance(g, R.Geometry.Brep):
                if g.Faces.Count != 1: raise ValueError('Weight created a polysurface')
                row['surface'] = host['_nurbs_surface_definition'](g.Surfaces[0])
            elif isinstance(g, R.Geometry.Surface): row['surface'] = host['_nurbs_surface_definition'](g)
            elif isinstance(g, R.Geometry.Mesh):
                row['mesh'] = host['_polygon_mesh_value'](g)
                row['mesh']['ngons'] = [dict(vertices=[int(i) for i in n.BoundaryVertexIndexList()],faces=[int(i) for i in n.FaceIndexList()]) for n in g.Ngons]
                row['mesh']['colors'] = [[int(c.R),int(c.G),int(c.B),int(c.A)] for c in g.VertexColors] if g.VertexColors.Count else None
            elif isinstance(g, R.Geometry.Point): row['point'] = host['_xyz'](g.Location)
            else: raise ValueError('unexpected Weight output geometry')
            rows.append(row)
        return dict(objects=rows, marker=doc.Objects.FindId(marker) is not None)

    def execute(name):
        host['_record_progress'](op['id'] + ' ' + name)
        before = snapshot(); history = R.RhinoApp.CommandHistoryWindowText
        state = None
        if name == 'Weight':
            timer, state = watch(host, op['inputs'], not op['dash'], op['pick'])
            try:
                _, after, events = observe_command(R.Commands.Command, name,
                    lambda: R.RhinoApp.RunScript('_-Weight' if op['dash'] else '_Weight', False), snapshot, lambda: [], True)
            finally: timer.Stop()
        else:
            _, after, events = observe_command(R.Commands.Command, name,
                lambda: R.RhinoApp.RunScript('_' + name, False), snapshot, lambda: [], True)
        results = [e['result'] for e in events if e['name'] == name]
        if len(results) != 1: raise ValueError('missing Weight workflow completion')
        if name == 'Weight':
            text = R.RhinoApp.CommandHistoryWindowText
            import json
            input_log.append(dict(input=state, history=text[len(history):] if text.startswith(history) else text[-3000:]))
            with open(path + '.weight.log', 'w') as stream:
                json.dump(input_log, stream, indent=2)
            if state['errors']: raise ValueError('Weight input failed: ' + repr(state))
        getter = dict(opened=state['applied'] or state['selection_opened'], selection=state['selection_opened'], default=state['default'], inputs=state['consumed']) if state else None
        return dict(command=name, before=before, after=after, after_script=snapshot(), result=results[0], getter=getter)

    try:
        serial = doc.BeginUndoRecord('Weight grip sources')
        try:
            for i, s in enumerate(op['sources']):
                g = host.get('_weight_source', create_source)(s['kind'], host)
                a = R.DocObjects.ObjectAttributes(); a.Name = names[i]
                a.ObjectColor = S.Drawing.Color.FromArgb(60 + 30 * i, 100, 180)
                a.ColorSource = R.DocObjects.ObjectColorSource.ColorFromObject
                try: key = doc.Objects.Add(g, a)
                finally: g.Dispose()
                if key == S.Guid.Empty: raise ValueError('cannot add Weight source')
                ids.append(key)
            a = R.DocObjects.ObjectAttributes(); a.Name = 'peer'; a.ObjectColor = S.Drawing.Color.FromArgb(8,16,24)
            a.ColorSource = R.DocObjects.ObjectColorSource.ColorFromObject
            peer = doc.Objects.AddPoint(host['_point']([20.,20.,20.]), a)
        finally: doc.EndUndoRecord(serial)
        model = R.FileIO.File3dm(); path = op['artifact_path']
        if path.startswith('/'): path = 'Z:' + path.replace('/', '\\')
        try:
            for key in ids + [peer]:
                obj = doc.Objects.FindId(key); model.Objects.Add(obj.Geometry, obj.Attributes)
            if not model.Write(path, 8): raise ValueError('cannot save Weight sources')
        finally: model.Dispose()
        group = doc.Groups.Add('WeightGrips_' + op['id'], S.Array[S.Guid](ids + [peer]))
        if group < 0: raise ValueError('cannot group Weight sources')
        for key, s in zip(ids, op['sources']):
            if s['mode'] == 'edit':
                doc.Objects.Select(key, True); R.RhinoApp.RunScript('_EditPtOn', False)
            else: doc.Objects.FindId(key).GripsOn = True
        doc.ClearUndoRecords(True); doc.ClearRedoRecords()
        serial = doc.BeginUndoRecord('Weight Redo witness')
        try: marker = doc.Objects.AddPoint(host['_point']([30.,30.,30.]))
        finally: doc.EndUndoRecord(serial)
        R.RhinoApp.RunScript('_Undo', False)
        doc.Objects.UnselectAll()
        for key, s in zip(ids, op['sources']):
            grips = list(doc.Objects.FindId(key).GetGrips() or [])
            if not grips: raise ValueError('missing initial Weight grips')
            for p in grips: p.Select(False)
            if s['hidden'] != 'empty':
                for i in picks(s['hidden'], len(grips)): grips[i].Select(True)
                R.RhinoApp.RunScript('_HidePt', False)
        for key, s in zip(ids, op['sources']):
            grips = list(doc.Objects.FindId(key).GetGrips())
            dimensions = None
            if s['selected'] in ('row_u', 'row_v', 'cross_rows'):
                g = doc.Objects.FindId(key).Geometry
                surface = g.Surfaces[0].ToNurbsSurface() if isinstance(g, R.Geometry.Brep) else g.ToNurbsSurface()
                try:
                    dimensions = [int(surface.Points.CountU), int(surface.Points.CountV)]
                    for axis in range(2):
                        if surface.IsPeriodic(axis): dimensions[axis] -= surface.Degree(axis)
                        elif surface.IsClosed(axis): dimensions[axis] -= 1
                    if dimensions[0] * dimensions[1] != len(grips): raise ValueError('Weight surface grip layout mismatch')
                finally: surface.Dispose()
            for i in picks(s['selected'], len(grips), dimensions): grips[i].Select(True)
            if s['selected'] == 'neighbor_hidden': R.RhinoApp.RunScript('_AddNextU', False)
            if s['selected'] == 'next_hidden': R.RhinoApp.RunScript('_NextU', False)
            if s['parent']: doc.Objects.Select(key, True)
        if op['peer']: doc.Objects.Select(peer, True)
        before = snapshot(); steps = [execute('Weight')]
        if op['repeat']: steps.append(execute('Weight'))
        if op['off']: steps.append(execute('PointsOff'))
        steps.extend([execute('Undo'), execute('Redo')])
        value = dict(before=before, steps=steps)
        import json
        with open(path + '.capture.json', 'w') as stream: json.dump(value, stream, indent=2, allow_nan=False)
        return value, 0
    finally:
        for obj in list(doc.Objects):
            if obj.GripsOn: obj.GripsOn = False
            doc.Objects.Delete(obj.Id, True)
        if group >= 0: doc.Groups.Delete(group)
        doc.ClearUndoRecords(True); doc.ClearRedoRecords()
