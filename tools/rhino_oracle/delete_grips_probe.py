"""Public Delete/Undo/Redo on selected logical grips in an owned document."""
import re
if __package__:
    from .grip_navigation_probe import KINDS as BASIC_KINDS, create_source as basic_source, seed_indices
else:
    from grip_navigation_probe import KINDS as BASIC_KINDS, create_source as basic_source, seed_indices

KINDS = BASIC_KINDS + ('cubic','quartic','nonuniform','weighted','periodic_cubic','closed_weighted',
    'surface_wide','surface_bicubic','mesh_fan','mesh_grid','mesh_box','polycurve_curved','mesh_colored','mesh_ngon','periodic_min')

SEEDS = ('first', 'last', 'middle', 'ends', 'all', 'empty', 'alternate', 'adjacent', 'except_first', 'except_ends',
    'row_u', 'row_v', 'cross_rows', 'neighbor_hidden', 'next_hidden', 'second', 'before_last')


def validate(op):
    if (not isinstance(op, dict) or set(op) != {'op', 'id', 'artifact_path', 'sources', 'peer', 'off'}
            or op.get('op') != 'delete_grips' or not isinstance(op.get('id'), str)
            or re.match(r'^[A-Za-z0-9_-]{1,80}\Z', op['id']) is None
            or not isinstance(op.get('artifact_path'), str)
            or not isinstance(op.get('sources'), list) or not 1 <= len(op['sources']) <= 4
            or type(op.get('peer')) is not bool or type(op.get('off')) is not bool):
        raise ValueError('invalid Delete grips recipe')
    for s in op['sources']:
        if (not isinstance(s, dict) or set(s) != {'kind', 'mode', 'selected', 'hidden', 'parent'}
                or s['kind'] not in KINDS or s['mode'] not in ('control', 'edit')
                or s['selected'] not in SEEDS or s['hidden'] not in ('empty', 'first', 'middle', 'all')
                or type(s['parent']) is not bool
                or s['mode'] == 'edit' and (s['kind'].startswith('surface') or s['kind'].startswith('mesh'))
                or s['selected'] in ('row_u', 'row_v', 'cross_rows') and not s['kind'].startswith('surface')
                or s['selected'] in ('neighbor_hidden','next_hidden') and (s['kind'] not in ('curve', 'rational', 'periodic') or s['mode'] != 'control' or s['hidden'] != 'middle')):
            raise ValueError('invalid Delete grip source')


def create_source(kind, host):
    if kind in BASIC_KINDS: return basic_source(kind, host)
    R = host['Rhino']
    if kind.startswith('mesh'):
        m = R.Geometry.Mesh(); m.Vertices.UseDoublePrecisionVertices = True
        if kind in ('mesh_fan','mesh_ngon'):
            vertices = [[0.,0.,0.],[-1.,-1.,0.],[1.,-1.,0.],[1.,1.,0.],[-1.,1.,0.]]
            faces = [[0,1,2],[0,2,3],[0,3,4],[0,4,1]]
        elif kind == 'mesh_colored':
            vertices = [[2.,0.,0.],[0.,2.,0.],[-2.,0.,0.],[0.,-2.,0.],[2.,0.,0.]]
            faces = [[0,1,2,3],[4,2,3]]
        elif kind == 'mesh_grid':
            vertices = [[float(i),float(j),0.] for j in range(4) for i in range(4)]
            faces = [[4*j+i,4*j+i+1,4*(j+1)+i+1,4*(j+1)+i] for j in range(3) for i in range(3)]
        else:
            vertices = [[float(i),float(j),float(k)] for k in range(2) for j in range(2) for i in range(2)]
            faces = [[0,2,3,1],[4,5,7,6],[0,1,5,4],[2,6,7,3],[0,4,6,2],[1,3,7,5]]
        for p in vertices: m.Vertices.Add(host['_point'](p))
        for f in faces: m.Faces.AddFace(*f)
        if kind == 'mesh_colored':
            S = host['System']
            for i in range(len(vertices)): m.VertexColors.Add(S.Drawing.Color.FromArgb(255,20+30*i,80,160))
        if kind == 'mesh_ngon':
            S = host['System']
            ngon = R.Geometry.MeshNgon.Create(S.Array[S.UInt32]([1,2,3,4]),S.Array[S.UInt32]([0,1,2,3]))
            m.Ngons.AddNgon(ngon)
        return m
    if kind.startswith('surface'):
        u, v, du, dv = (6,5,3,2) if kind == 'surface_wide' else (5,6,3,3)
        s = R.Geometry.NurbsSurface.Create(3,True,du+1,dv+1,u,v)
        for i in range(u):
            for j in range(v): s.Points.SetPoint(i,j,host['_point']([float(i),float(j),.1*i*i-.2*i*j+.05*j*j]),1.+.1*i+.05*j)
        s.KnotsU.CreateUniformKnots(1.); s.KnotsV.CreateUniformKnots(1.)
        if kind == 'surface_wide':
            host['_set_knots'](s.KnotsU,[0.]*4+[1.,3.]+[5.]*4,'Delete U knots')
            host['_set_knots'](s.KnotsV,[0.]*3+[.75,2.]+[4.]*3,'Delete V knots')
        if not s.IsValid: raise ValueError('invalid Delete surface')
        return s
    if kind == 'polycurve_curved':
        c = R.Geometry.PolyCurve(); curve = create_source('weighted',host)
        a = R.Geometry.LineCurve(host['_point']([-2.,0.,0.]),curve.PointAtStart)
        b = R.Geometry.LineCurve(curve.PointAtEnd,host['_point']([12.,0.,0.]))
        try:
            if not c.Append(a) or not c.Append(curve) or not c.Append(b): raise ValueError('cannot build Delete polycurve')
        finally: a.Dispose(); b.Dispose(); curve.Dispose()
        return c
    degree = 4 if kind in ('quartic','nonuniform') else 3
    periodic = kind in ('periodic_cubic','periodic_min'); closed = kind == 'closed_weighted'
    unique = 3 if kind == 'periodic_min' else 7 if periodic or closed else degree + 5
    import math
    points = [[math.cos(2.*math.pi*i/unique)*3.,math.sin(2.*math.pi*i/unique)*3.,0.] for i in range(unique)] if periodic or closed else [[float(i),float(i%3-1)*1.5,float(i%2)*.25] for i in range(unique)]
    points += points[:degree] if periodic else points[:1] if closed else []
    c = R.Geometry.NurbsCurve(3,True,degree+1,len(points))
    for i,p in enumerate(points):c.Points.SetPoint(i,host['_point'](p),[.7,1.6,.9,2.2][i%4] if kind in ('weighted','closed_weighted') else 1.)
    if periodic:c.Knots.CreatePeriodicKnots(1.)
    else:c.Knots.CreateUniformKnots(1.)
    if kind == 'nonuniform':
        interior = [float(i*i)*.2 for i in range(1,len(points)-degree)]
        host['_set_knots'](c.Knots,[0.]*(degree+1)+interior+[interior[-1]+2.]*(degree+1),'Delete curve knots')
    if not c.IsValid: raise ValueError('invalid Delete curve')
    return c


def picks(seed, count, dimensions=None):
    if count == 0: return []
    if seed == 'alternate': return list(range(0, count, 2))
    if seed == 'second': return [min(1,count-1)]
    if seed == 'before_last': return [max(0,count-2)]
    if seed == 'adjacent': return sorted(set([max(0, count // 2 - 1), count // 2]))
    if seed == 'except_first': return list(range(1,count))
    if seed == 'except_ends': return list(range(1,count-1))
    if seed in ('neighbor_hidden','next_hidden'): return [max(0, count // 2 - 1)]
    if seed in ('row_u', 'row_v', 'cross_rows'):
        u, v = dimensions
        if seed == 'cross_rows': return sorted(set(picks('row_u',count,dimensions)+picks('row_v',count,dimensions)))
        return [i * v + v // 2 for i in range(u)] if seed == 'row_u' else [u // 2 * v + j for j in range(v)]
    return seed_indices(seed, count)


def run(op, host):
    validate(op)
    from join_probe import observe_command
    R, S = host['Rhino'], host['System']; doc = R.RhinoDoc.ActiveDoc
    if list(doc.Objects) or R.Commands.Command.InCommand(): raise ValueError('Delete capture needs an empty owned idle document')
    ids = []; group = -1; peer = marker = S.Guid.Empty
    names = ['source' + str(i) for i in range(len(op['sources']))] + ['peer']

    def snapshot():
        rows = []; seen = set(); handles = ids + [peer]
        for obj in sorted(list(doc.Objects), key=lambda o: o.RuntimeSerialNumber):
            if obj.Id == marker: continue
            name = obj.Attributes.Name
            if name not in names or name in seen: raise ValueError('unexpected Delete output identity')
            seen.add(name); index = names.index(name); g = obj.Geometry
            memberships = list(obj.Attributes.GetGroupList() or [])
            if any(i != group for i in memberships): raise ValueError('unexpected Delete group')
            row = dict(role=name, same_id=obj.Id == handles[index], name=name,
                color=[int(obj.Attributes.ObjectColor.R), int(obj.Attributes.ObjectColor.G), int(obj.Attributes.ObjectColor.B)],
                grouped=group in memberships, selected=bool(obj.IsSelected(False)), visible=not bool(obj.IsHidden),
                grips_on=bool(obj.GripsOn), kind=g.GetType().Name,
                grips=[dict(index=int(p.Index), point=host['_xyz'](p.CurrentLocation), selected=bool(p.IsSelected(False)),
                    hidden=bool(p.IsHidden), selectable=bool(p.IsSelectable(True,False,False,False))) for p in (obj.GetGrips() or [])])
            if isinstance(g, R.Geometry.Curve): row['curve'] = host['_nurbs_curve_definition'](g)
            elif isinstance(g, R.Geometry.Brep):
                if g.Faces.Count != 1: raise ValueError('Delete created a polysurface')
                row['surface'] = host['_nurbs_surface_definition'](g.Surfaces[0])
            elif isinstance(g, R.Geometry.Surface): row['surface'] = host['_nurbs_surface_definition'](g)
            elif isinstance(g, R.Geometry.Mesh):
                row['mesh'] = host['_polygon_mesh_value'](g)
                row['mesh']['ngons'] = [dict(vertices=[int(i) for i in n.BoundaryVertexIndexList()],faces=[int(i) for i in n.FaceIndexList()]) for n in g.Ngons]
                row['mesh']['colors'] = [[int(c.R),int(c.G),int(c.B),int(c.A)] for c in g.VertexColors] if g.VertexColors.Count else None
            elif isinstance(g, R.Geometry.Point): row['point'] = host['_xyz'](g.Location)
            else: raise ValueError('unexpected Delete output geometry')
            rows.append(row)
        return dict(objects=rows, marker=doc.Objects.FindId(marker) is not None)

    def execute(name):
        host['_record_progress'](op['id'] + ' ' + name)
        before = snapshot()
        history = R.RhinoApp.CommandHistoryWindowText if name == 'Delete' else None
        _, after, events = observe_command(R.Commands.Command, name, lambda: R.RhinoApp.RunScript('_' + name, False), snapshot, lambda: [], True)
        results = [e['result'] for e in events if e['name'] == name]
        if len(results) != 1: raise ValueError('missing Delete workflow completion')
        if history is not None:
            text = R.RhinoApp.CommandHistoryWindowText
            with open(path + '.delete.log','w') as stream: stream.write(text[len(history):] if text.startswith(history) else text[-3000:])
        return dict(command=name, before=before, after=after, after_script=snapshot(), result=results[0])

    try:
        serial = doc.BeginUndoRecord('Delete grip sources')
        try:
            for i, s in enumerate(op['sources']):
                g = create_source(s['kind'], host)
                a = R.DocObjects.ObjectAttributes(); a.Name = names[i]
                a.ObjectColor = S.Drawing.Color.FromArgb(60 + 30 * i, 100, 180)
                a.ColorSource = R.DocObjects.ObjectColorSource.ColorFromObject
                try: key = doc.Objects.Add(g, a)
                finally: g.Dispose()
                if key == S.Guid.Empty: raise ValueError('cannot add Delete source')
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
            if not model.Write(path, 8): raise ValueError('cannot save Delete sources')
        finally: model.Dispose()
        group = doc.Groups.Add('DeleteGrips_' + op['id'], S.Array[S.Guid](ids + [peer]))
        if group < 0: raise ValueError('cannot group Delete sources')
        for key, s in zip(ids, op['sources']):
            if s['mode'] == 'edit':
                doc.Objects.Select(key, True); R.RhinoApp.RunScript('_EditPtOn', False)
            else: doc.Objects.FindId(key).GripsOn = True
        doc.ClearUndoRecords(True); doc.ClearRedoRecords()
        serial = doc.BeginUndoRecord('Delete Redo witness')
        try: marker = doc.Objects.AddPoint(host['_point']([30.,30.,30.]))
        finally: doc.EndUndoRecord(serial)
        R.RhinoApp.RunScript('_Undo', False)
        doc.Objects.UnselectAll()
        for key, s in zip(ids, op['sources']):
            grips = list(doc.Objects.FindId(key).GetGrips() or [])
            if not grips: raise ValueError('missing initial Delete grips')
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
                    if dimensions[0] * dimensions[1] != len(grips): raise ValueError('Delete surface grip layout mismatch')
                finally: surface.Dispose()
            for i in picks(s['selected'], len(grips), dimensions): grips[i].Select(True)
            if s['selected'] == 'neighbor_hidden': R.RhinoApp.RunScript('_AddNextU', False)
            if s['selected'] == 'next_hidden': R.RhinoApp.RunScript('_NextU', False)
            if s['parent']: doc.Objects.Select(key, True)
        if op['peer']: doc.Objects.Select(peer, True)
        before = snapshot(); steps = [execute('Delete')]
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
