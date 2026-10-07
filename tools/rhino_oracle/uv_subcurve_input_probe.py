"""Closed UV-command recipes using public inline SubCrv getters."""
import re

CASES = ('apply_forward', 'apply_reverse', 'apply_rectangle', 'apply_mixed',
         'create_forward', 'create_reverse', 'create_warped', 'create_mixed',
         'apply_twice', 'create_twice', 'apply_closed', 'create_closed',
         'apply_clear', 'create_clear')


def validate_request(q):
    if (type(q.get('protocol_version')) is not int or q['protocol_version'] != 1
            or type(q.get('iterations', 1)) is not int or q.get('iterations', 1) != 1
            or not isinstance(q.get('operations'), list) or not 1 <= len(q['operations']) <= 16):
        raise ValueError('UV SubCrv requires bounded protocol 1 recipes')
    seen = set()
    for op in q['operations']:
        if (not isinstance(op, dict) or set(op) != {'op', 'id', 'case'}
                or op['op'] != 'uv_subcurve_input_command' or not isinstance(op['id'], str)
                or re.match(r'^[A-Za-z0-9_-]{1,80}\Z', op['id']) is None
                or op['id'] in seen or op.get('case') not in CASES):
            raise ValueError('invalid UV SubCrv recipe')
        seen.add(op['id'])


def request():
    return dict(protocol_version=1, iterations=1, operations=[
        dict(op='uv_subcurve_input_command', id='uv_subcurve_'+case, case=case) for case in CASES])


def run(op, host):
    validate_request(dict(protocol_version=1, operations=[op]))
    Rhino, System = host['Rhino'], host['System']
    G, doc = Rhino.Geometry, Rhino.RhinoDoc.ActiveDoc
    if Rhino.Commands.Command.InCommand(): raise ValueError('UV SubCrv requires idle execution')
    if list(doc.Objects): raise ValueError('UV SubCrv requires an empty owned document')
    from join_probe import observe_command
    owned, ids = [], []
    saved_tolerance = doc.ModelAbsoluteTolerance
    baseline_groups = set(i for i in range(doc.Groups.Count) if not doc.Groups.IsDeleted(i))
    layers = []
    doc.ModelAbsoluteTolerance = 1e-6

    def keep(g):
        if g is None or not g.IsValid: raise ValueError('invalid UV SubCrv source')
        owned.append(g)
        return g

    def attrs(name):
        a = doc.CreateDefaultAttributes()
        a.Name = name
        a.LayerIndex = layers[0]
        a.SetUserString('viboceros-source', name)
        return a

    def snapshot():
        rows = []
        for obj in sorted(list(doc.Objects), key=lambda o: o.RuntimeSerialNumber):
            g, a = obj.Geometry, obj.Attributes
            row = dict(source=ids.index(obj.Id) if obj.Id in ids else None,
                       name=a.Name, user_text=a.GetUserString('viboceros-source'),
                       layer=doc.Layers[a.LayerIndex].Name,
                       groups=list(a.GetGroupList() or []), selected=bool(obj.IsSelected(False)))
            if isinstance(g, G.Curve):
                n = g.ToNurbsCurve()
                try:
                    row.update(kind='curve', definition=host['_nurbs_curve_definition'](n),
                               samples=[host['_xyz'](g.PointAt(g.Domain.ParameterAt(i/32.))) for i in range(33)])
                finally: n.Dispose()
            elif isinstance(g, G.Point): row.update(kind='point', point=host['_xyz'](g.Location))
            else:
                underlying = g.Faces[0].UnderlyingSurface() if isinstance(g, G.Brep) else g
                n = underlying.ToNurbsSurface()
                try: row.update(kind='surface', definition=host['_nurbs_surface_definition'](n))
                finally: n.Dispose()
            rows.append(row)
        return rows

    def groups():
        return [dict(index=i, name=doc.Groups.GroupName(i)) for i in range(doc.Groups.Count)
                if i not in baseline_groups and not doc.Groups.IsDeleted(i)]

    try:
        layer = Rhino.DocObjects.Layer()
        layer.Name = op['id']+'_inputs'
        layers.append(doc.Layers.Add(layer))
        case = op['case']
        apply = case.startswith('apply_')
        height = 2. if case == 'create_warped' else 0.
        surface = keep(host['_nurbs_surface_from_definition'](dict(
            degree_u=1, degree_v=1, control_point_count_u=2, control_point_count_v=2,
            control_points=[dict(point=p, weight=1.) for p in
                ((0.,0.,0.), (4.,0.,0.), (0.,6.,0.), (4.,6.,height))],
            knots_u=[2.,2.,5.,5.], knots_v=[-3.,-3.,1.,1.])))
        ids.append(doc.Objects.AddSurface(surface, attrs('target')))
        if case.endswith('closed'):
            source = keep(G.PolylineCurve([G.Point3d(x,y,0.) for x,y in
                ((0.,0.), (4.,0.), (4.,6.), (0.,6.), (0.,0.))]))
        elif height:
            source = keep(host['_nurbs_curve_from_definition'](dict(degree=2,
                knots=[2.,2.,2.,5.,5.,5.], control_points=[dict(point=p,weight=1.)
                for p in ((0.,0.,0.), (2.,3.,0.), (4.,6.,2.))])))
        else: source = keep(G.LineCurve(G.Point3d(0.,0.,0.), G.Point3d(4.,6.,0.)))
        ids.append(doc.Objects.AddCurve(source, attrs('partial')))
        doc.Groups.Add('UV_SubCrv_source_'+op['id'], [ids[1]])
        fractions = [.8,.15] if case.endswith('closed') else ([.75,.25] if case.endswith('reverse') else [.25,.75])
        parameters = [source.Domain.ParameterAt(t) for t in fractions]
        picks = [host['_xyz'](source.PointAt(t)) for t in parameters]
        extra_ids = []
        if case in ('apply_rectangle', 'apply_mixed', 'apply_clear'):
            rectangle = keep(G.PolylineCurve([G.Point3d(x,y,0.) for x,y in
                ((0.,0.), (4.,0.), (4.,6.), (0.,6.), (0.,0.))]))
            ids.append(doc.Objects.AddCurve(rectangle, attrs('rectangle')))
            extra_ids.append(ids[-1])
        if case.endswith('mixed'):
            ids.append(doc.Objects.AddPoint(G.Point3d(2.,3.,0.), attrs('point')))
            extra_ids.append(ids[-1])
        if extra_ids: doc.Groups.Add('UV_SubCrv_whole_'+op['id'], extra_ids)
        shortened = '_SubCrv _SelID '+str(ids[1])+' '+' '.join(
            ','.join(repr(x) for x in p) for p in picks)
        ranges = [parameters]
        if case.endswith('twice'):
            second = [source.Domain.ParameterAt(t) for t in (.4,.6)]
            second_picks = [host['_xyz'](source.PointAt(t)) for t in second]
            ranges.append(second)
            shortened += ' _SubCrv _SelID '+str(ids[1])+' '+' '.join(
                ','.join(repr(x) for x in p) for p in second_picks)
        if case.endswith('clear'): shortened += ' _SelNone'
        extras = ' '.join('_SelID '+str(i) for i in extra_ids)
        command = 'ApplyCrv' if apply else 'CreateUVCrv'
        macro = ('_ApplyCrv '+shortened+' '+extras+' _Enter _SelID '+str(ids[0])+' _Enter' if apply
                 else '_CreateUVCrv _SelID '+str(ids[0])+' '+shortened+' '+extras+' _Enter')
        doc.ClearUndoRecords(True)
        before, groups_before = snapshot(), groups()
        marker = 'Viboceros UV SubCrv '+str(System.Guid.NewGuid())
        Rhino.RhinoApp.WriteLine(marker)
        success, after, events = observe_command(Rhino.Commands.Command, command,
            lambda: Rhino.RhinoApp.RunScript(macro, True), snapshot, lambda: [], True)
        active = bool(Rhino.Commands.Command.InCommand())
        if active: Rhino.RhinoApp.RunScript('!', False)
        after_script, groups_after = snapshot(), groups()
        history = Rhino.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1]
        Rhino.RhinoApp.RunScript('_Undo',False)
        undo, groups_undo = snapshot(), groups()
        Rhino.RhinoApp.RunScript('_Redo',False)
        redo, groups_redo = snapshot(), groups()
        return dict(command=command, macro=macro, parameters=parameters, ranges=ranges, picks=picks,
            before=before, after=after, after_script=after_script, success=success,
            command_active=active, events=events, history=history, undo=undo, redo=redo,
            groups_before=groups_before, groups_after=groups_after,
            groups_undo=groups_undo, groups_redo=groups_redo, tolerance=1e-6), 0
    finally:
        if Rhino.Commands.Command.InCommand(): Rhino.RhinoApp.RunScript('!',False)
        doc.Objects.UnselectAll()
        for obj in list(doc.Objects): doc.Objects.Delete(obj.Id,True)
        for i in range(doc.Groups.Count):
            if i not in baseline_groups and not doc.Groups.IsDeleted(i): doc.Groups.Delete(i)
        for i in reversed(layers): doc.Layers.Delete(i,True)
        doc.ModelAbsoluteTolerance = saved_tolerance
        for g in reversed(owned): g.Dispose()
