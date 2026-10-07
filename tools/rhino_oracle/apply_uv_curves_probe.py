"""Closed public ApplyCrv command recipes on an owned empty document."""
import re

CASES = ('rectangle', 'without_rectangle', 'points', 'horizontal', 'near_xy',
         'off_xy', 'rational', 'preselection', 'rational_without_rectangle',
         'off_xy_outside', 'single_point', 'cylinder', 'rotated_cplane')


def validate_request(q):
    if (type(q.get('protocol_version')) is not int or q['protocol_version'] != 1
            or type(q.get('iterations', 1)) is not int or q.get('iterations', 1) != 1
            or not isinstance(q.get('operations'), list) or not 1 <= len(q['operations']) <= 32):
        raise ValueError('ApplyCrv requires bounded protocol 1 recipes')
    seen = set()
    for op in q['operations']:
        if (not isinstance(op, dict) or set(op) != {'op', 'id', 'case'}
                or op['op'] != 'apply_uv_curves_command' or not isinstance(op['id'], str)
                or re.match(r'^[A-Za-z0-9_-]{1,80}\Z', op['id']) is None
                or op['id'] in seen or op.get('case') not in CASES):
            raise ValueError('invalid ApplyCrv recipe')
        seen.add(op['id'])


def request():
    return dict(protocol_version=1, iterations=1, operations=[
        dict(op='apply_uv_curves_command', id='apply_uv_'+case, case=case) for case in CASES])


def run(op, host):
    validate_request(dict(protocol_version=1, operations=[op]))
    Rhino, System = host['Rhino'], host['System']
    if Rhino.Commands.Command.InCommand():
        raise ValueError('ApplyCrv requires idle execution')
    doc, G = Rhino.RhinoDoc.ActiveDoc, Rhino.Geometry
    if list(doc.Objects):
        raise ValueError('ApplyCrv requires an empty owned document')
    from join_probe import observe_command
    owned, ids, layers, groups = [], [], [], []
    saved_layer, saved_tolerance = doc.Layers.CurrentLayerIndex, doc.ModelAbsoluteTolerance
    baseline_groups = set(i for i in range(doc.Groups.Count) if not doc.Groups.IsDeleted(i))
    viewport = doc.Views.ActiveView.ActiveViewport
    saved_plane = viewport.GetConstructionPlane().Plane
    doc.ModelAbsoluteTolerance = 1e-6
    case = op['case']
    if case == 'rotated_cplane': viewport.SetConstructionPlane(G.Plane.WorldYZ)

    def keep(g):
        if g is None or not g.IsValid:
            raise ValueError('invalid ApplyCrv source')
        owned.append(g)
        return g

    def attrs(name, layer):
        a = doc.CreateDefaultAttributes()
        a.Name, a.LayerIndex = name, layer
        a.SetUserString('viboceros-source', name)
        a.AddToGroup(groups[0])
        return a

    def curve(points, degree=1, weights=None):
        return keep(host['_nurbs_curve_from_definition'](dict(
            degree=degree, knots=[2.]*(degree+1)+[5.]*(degree+1),
            control_points=[dict(point=p, weight=w) for p, w in zip(points, weights or [1.]*len(points))])))

    def snapshot():
        rows = []
        for obj in sorted(list(doc.Objects), key=lambda x: x.RuntimeSerialNumber):
            g, a = obj.Geometry, obj.Attributes
            row = dict(source=ids.index(obj.Id) if obj.Id in ids else None, name=a.Name,
                       layer=doc.Layers[a.LayerIndex].Name, groups=list(a.GetGroupList() or []),
                       user_text=a.GetUserString('viboceros-source'), selected=bool(obj.IsSelected(False)))
            if isinstance(g, G.Curve):
                n = g.ToNurbsCurve()
                try:
                    row.update(kind='curve', definition=host['_nurbs_curve_definition'](n),
                               samples=[host['_xyz'](g.PointAt(g.Domain.ParameterAt(i/32.))) for i in range(33)])
                finally:
                    n.Dispose()
            elif isinstance(g, G.Point):
                row.update(kind='point', point=host['_xyz'](g.Location))
            else:
                underlying = g.Faces[0].UnderlyingSurface() if isinstance(g,G.Brep) else g
                n = underlying.ToNurbsSurface()
                try: row.update(kind='surface',definition=host['_nurbs_surface_definition'](n))
                finally: n.Dispose()
            rows.append(row)
        return rows

    def group_records():
        return [dict(index=i,name=doc.Groups.GroupName(i)) for i in range(doc.Groups.Count)
                if not doc.Groups.IsDeleted(i) and i not in baseline_groups]

    try:
        groups.append(doc.Groups.Add('ApplyUV_source_'+op['id']))
        for suffix in ('input', 'output'):
            layer = Rhino.DocObjects.Layer()
            layer.Name = op['id']+'_'+suffix
            layers.append(doc.Layers.Add(layer))
        s = keep(host['_nurbs_surface_from_definition'](dict(
            degree_u=1, degree_v=1, control_point_count_u=2, control_point_count_v=2,
            control_points=[dict(point=p, weight=1.) for p in
                            ((0.,0.,0.), (4.,0.,0.), (0.,6.,0.), (4.,6.,2.))],
            knots_u=[2.,2.,5.,5.], knots_v=[-3.,-3.,1.,1.])))
        if case == 'cylinder':
            s = keep(G.Cylinder(G.Circle(G.Plane.WorldXY,2.),3.).ToNurbsSurface())
        target = doc.Objects.AddSurface(s, attrs('target', layers[0]))
        ids.append(target)
        inputs = []
        if case not in ('without_rectangle', 'points', 'horizontal', 'single_point', 'rational_without_rectangle'):
            rect = keep(G.PolylineCurve([G.Point3d(x,y,0.) for x,y in
                                        ((10.,20.),(14.,20.),(14.,26.),(10.,26.),(10.,20.))]))
            inputs.append(('rectangle', rect))
        if case == 'single_point':
            inputs.append(('point', G.Point3d(12.,23.,0.)))
        elif case == 'points':
            inputs.extend([('point_a', G.Point3d(11.,21.5,0.)), ('point_b', G.Point3d(13.,24.5,0.))])
        elif case == 'horizontal':
            inputs.append(('line', curve([(10.,20.,0.),(14.,20.,0.)])))
        elif case.startswith('rational'):
            inputs.append(('rational', curve([(11.,21.5,0.),(12.,24.5,0.),(13.,21.5,0.)],2,[1.,2.,1.])))
        else:
            z = 5e-7 if case == 'near_xy' else .01 if case == 'off_xy' else 0.
            inputs.append(('line', curve([(11.,21.5,z),(13.,24.5,z)])))
            inputs.append(('point', G.Point3d(12.,23.,z)))
            if case == 'off_xy_outside':
                inputs.append(('outside', curve([(50.,60.,.01),(70.,80.,.01)])))
        definitions = []
        for name, g in inputs:
            if isinstance(g, G.Point3d):
                oid = doc.Objects.AddPoint(g, attrs(name, layers[0]))
                definitions.append(dict(kind='point', name=name, point=host['_xyz'](g)))
            else:
                oid = doc.Objects.AddCurve(g, attrs(name, layers[0]))
                n = g.ToNurbsCurve()
                try: definitions.append(dict(kind='curve', name=name, definition=host['_nurbs_curve_definition'](n)))
                finally: n.Dispose()
            if oid == System.Guid.Empty: raise ValueError('ApplyCrv source insertion failed')
            ids.append(oid)
        doc.Layers.SetCurrentLayerIndex(layers[1], True)
        # Each recipe owns a fresh history baseline. A successful no-output
        # command must not accidentally Undo an earlier recipe's command.
        doc.ClearUndoRecords(True)
        if case == 'preselection':
            for oid in ids[1:]: doc.Objects.Select(oid)
            macro = '_ApplyCrv _SelID '+str(target)+' _Enter'
        else:
            selectors = ' '.join('_SelID '+str(oid) for oid in ids[1:])
            macro = '_ApplyCrv '+selectors+' _Enter _SelID '+str(target)+' _Enter'
        before = snapshot()
        groups_before = group_records()
        marker = 'Viboceros ApplyCrv '+str(System.Guid.NewGuid())
        Rhino.RhinoApp.WriteLine(marker)
        success, after, events = observe_command(Rhino.Commands.Command, 'ApplyCrv',
            lambda: Rhino.RhinoApp.RunScript(macro, True), snapshot, lambda: [], True)
        active = bool(Rhino.Commands.Command.InCommand())
        if active: Rhino.RhinoApp.RunScript('!', False)
        after_script = snapshot()
        groups_after = group_records()
        history = Rhino.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1]
        Rhino.RhinoApp.RunScript('_Undo',False)
        undo = snapshot()
        groups_undo = group_records()
        undo_history = Rhino.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1]
        Rhino.RhinoApp.RunScript('_Redo',False)
        redo = snapshot()
        groups_redo = group_records()
        return dict(surface=host['_nurbs_surface_definition'](s), inputs=definitions, before=before,
                    success=success, command_active=active, after=after, after_script=after_script,
                    events=events, history=history, undo=undo, redo=redo, tolerance=1e-6,
                    groups_before=groups_before,groups_after=groups_after,groups_undo=groups_undo,
                    groups_redo=groups_redo,undo_history=undo_history,history_cleared_before_command=True), 0
    finally:
        if Rhino.Commands.Command.InCommand(): Rhino.RhinoApp.RunScript('!',False)
        doc.Objects.UnselectAll()
        for obj in list(doc.Objects): doc.Objects.Delete(obj.Id,True)
        doc.Layers.SetCurrentLayerIndex(saved_layer,True)
        for i in range(doc.Groups.Count):
            if i not in baseline_groups and not doc.Groups.IsDeleted(i): doc.Groups.Delete(i)
        for layer in reversed(layers): doc.Layers.Delete(layer,True)
        doc.ModelAbsoluteTolerance = saved_tolerance
        viewport.SetConstructionPlane(saved_plane)
        for g in reversed(owned): g.Dispose()
