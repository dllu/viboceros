"""Closed public CreateUVCrv recipes, isolated document/history ownership."""
import re

CASES = ('warped', 'planar', 'cylinder', 'sphere', 'extras', 'off_point',
         'off_curve', 'preselection', 'rotated_cplane','trim_warped','holed_plane')


def validate_request(q):
    if (type(q.get('protocol_version')) is not int or q['protocol_version'] != 1
            or type(q.get('iterations', 1)) is not int or q.get('iterations', 1) != 1
            or not isinstance(q.get('operations'), list) or not 1 <= len(q['operations']) <= 32):
        raise ValueError('CreateUVCrv requires bounded protocol 1 recipes')
    seen = set()
    for op in q['operations']:
        if (not isinstance(op, dict) or set(op) != {'op', 'id', 'case'}
                or op['op'] != 'create_uv_curves_command' or not isinstance(op['id'], str)
                or re.match(r'^[A-Za-z0-9_-]{1,80}\Z', op['id']) is None
                or op['id'] in seen or op.get('case') not in CASES):
            raise ValueError('invalid CreateUVCrv recipe')
        seen.add(op['id'])


def request():
    return dict(protocol_version=1, iterations=1, operations=[
        dict(op='create_uv_curves_command', id='create_uv_'+case, case=case) for case in CASES])


def run(op, host):
    validate_request(dict(protocol_version=1, operations=[op]))
    Rhino, System = host['Rhino'], host['System']
    if Rhino.Commands.Command.InCommand(): raise ValueError('CreateUVCrv requires idle execution')
    doc, G = Rhino.RhinoDoc.ActiveDoc, Rhino.Geometry
    if list(doc.Objects): raise ValueError('CreateUVCrv requires an empty owned document')
    from join_probe import observe_command
    owned, ids, layers = [], [], []
    baseline_groups = set(i for i in range(doc.Groups.Count) if not doc.Groups.IsDeleted(i))
    saved_layer, saved_tolerance = doc.Layers.CurrentLayerIndex, doc.ModelAbsoluteTolerance
    viewport = doc.Views.ActiveView.ActiveViewport
    saved_plane = viewport.GetConstructionPlane().Plane
    doc.ModelAbsoluteTolerance = 1e-6
    case = op['case']

    def keep(g):
        if g is None or not g.IsValid: raise ValueError('invalid CreateUVCrv source')
        owned.append(g)
        return g

    def attrs(name):
        a = doc.CreateDefaultAttributes()
        a.Name, a.LayerIndex = name, layers[0]
        a.SetUserString('viboceros-source', name)
        a.AddToGroup(group)
        return a

    def snapshot():
        rows = []
        for obj in sorted(list(doc.Objects), key=lambda x: x.RuntimeSerialNumber):
            g,a = obj.Geometry,obj.Attributes
            row = dict(source=ids.index(obj.Id) if obj.Id in ids else None,name=a.Name,
                       layer=doc.Layers[a.LayerIndex].Name,groups=list(a.GetGroupList() or []),
                       user_text=a.GetUserString('viboceros-source'),selected=bool(obj.IsSelected(False)))
            if isinstance(g,G.Curve):
                n = g.ToNurbsCurve()
                try: row.update(kind='curve',definition=host['_nurbs_curve_definition'](n),
                    samples=[host['_xyz'](g.PointAt(g.Domain.ParameterAt(i/32.))) for i in range(33)])
                finally: n.Dispose()
            elif isinstance(g,G.Point): row.update(kind='point',point=host['_xyz'](g.Location))
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
        group = doc.Groups.Add('CreateUV_source_'+op['id'])
        for suffix in ('input','output'):
            layer = Rhino.DocObjects.Layer()
            layer.Name = op['id']+'_'+suffix
            layers.append(doc.Layers.Add(layer))
        height = 0. if case in ('planar','holed_plane') else 2.
        s = keep(host['_nurbs_surface_from_definition'](dict(
            degree_u=1,degree_v=1,control_point_count_u=2,control_point_count_v=2,
            control_points=[dict(point=p,weight=1.) for p in
                ((0.,0.,0.),(4.,0.,0.),(0.,6.,0.),(4.,6.,height))],
            knots_u=[2.,2.,5.,5.],knots_v=[-3.,-3.,1.,1.])))
        if case == 'cylinder': s = keep(G.Cylinder(G.Circle(G.Plane.WorldXY,2.),3.).ToNurbsSurface())
        if case == 'sphere': s = keep(G.Sphere(G.Point3d.Origin,2.).ToNurbsSurface())
        target_brep = None
        if case in ('trim_warped','holed_plane'):
            b = keep(G.Brep.CreateFromSurface(s))
            if case == 'trim_warped':
                uv = keep(G.PolylineCurve([G.Point3d(x,y,0.) for x,y in
                    ((2.5,-2.),(4.5,-2.),(4.5,0.),(2.5,0.),(2.5,-2.))]))
            else:
                uv = keep(G.Circle(G.Plane(G.Point3d(3.5,-1.,0.),G.Vector3d.ZAxis),.4).ToNurbsCurve())
            spatial = keep(s.Pushup(uv,1e-8))
            split = keep(b.Faces[0].Split([spatial],1e-6))
            candidates = [keep(f.DuplicateFace(False)) for f in split.Faces]
            def area(g):
                mp = G.AreaMassProperties.Compute(g)
                try:return float(mp.Area)
                finally:mp.Dispose()
            target_brep = min(candidates,key=area) if case == 'trim_warped' else max(candidates,key=area)
            s = keep(target_brep.Faces[0].UnderlyingSurface().ToNurbsSurface())
        ids.append(doc.Objects.AddBrep(target_brep,attrs('target')) if target_brep is not None else doc.Objects.AddSurface(s,attrs('target')))
        trim_loops = []
        if target_brep is not None:
            for loop in target_brep.Faces[0].Loops:
                curves = []
                for trim in loop.Trims:
                    n = trim.ToNurbsCurve()
                    try:curves.append(host['_nurbs_curve_definition'](n))
                    finally:n.Dispose()
                trim_loops.append(dict(type=str(loop.LoopType),curves=curves))
        iso_lengths = []
        for direction in (0,1):
            row = []
            for i in range(9):
                c = s.IsoCurve(direction,s.Domain(1-direction).ParameterAt(i/8.))
                try: row.append([float(c.GetLength())]+[float(c.GetLength(t)) for t in (.01,.001,.0001)])
                finally: c.Dispose()
            iso_lengths.append(row)
        inputs = []
        if case in ('extras','off_curve','preselection','rotated_cplane'):
            offset = .01 if case == 'off_curve' else 0.
            c = keep(host['_nurbs_curve_from_definition'](dict(degree=2,knots=[2.,2.,2.,5.,5.,5.],
                control_points=[dict(point=p,weight=1.) for p in
                    ((0.,0.,offset),(2.,3.,offset),(4.,6.,2.+offset))])))
            ids.append(doc.Objects.AddCurve(c,attrs('diagonal')))
            inputs.append(dict(kind='curve',name='diagonal',definition=host['_nurbs_curve_definition'](c)))
        if case in ('extras','off_point','preselection','rotated_cplane'):
            p = G.Point3d(2.,3.,10. if case == 'off_point' else .5)
            ids.append(doc.Objects.AddPoint(p,attrs('point')))
            inputs.append(dict(kind='point',name='point',point=host['_xyz'](p)))
        doc.Layers.SetCurrentLayerIndex(layers[1],True)
        doc.ClearUndoRecords(True)
        if case == 'rotated_cplane': viewport.SetConstructionPlane(G.Plane.WorldYZ)
        if case == 'preselection':
            doc.Objects.Select(ids[0])
            macro = '_CreateUVCrv '+' '.join('_SelID '+str(i) for i in ids[1:])+' _Enter'
        else:
            macro = '_CreateUVCrv _SelID '+str(ids[0])+' '+' '.join('_SelID '+str(i) for i in ids[1:])+' _Enter'
        before,groups_before = snapshot(),group_records()
        marker = 'Viboceros CreateUVCrv '+str(System.Guid.NewGuid())
        Rhino.RhinoApp.WriteLine(marker)
        success,after,events = observe_command(Rhino.Commands.Command,'CreateUVCrv',
            lambda:Rhino.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        active = bool(Rhino.Commands.Command.InCommand())
        if active: Rhino.RhinoApp.RunScript('!',False)
        after_script,groups_after = snapshot(),group_records()
        history = Rhino.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1]
        Rhino.RhinoApp.RunScript('_Undo',False)
        undo,groups_undo = snapshot(),group_records()
        Rhino.RhinoApp.RunScript('_Redo',False)
        redo,groups_redo = snapshot(),group_records()
        return dict(surface=host['_nurbs_surface_definition'](s),trim_loops=trim_loops,surface_size=list(s.GetSurfaceSize()),iso_lengths=iso_lengths,inputs=inputs,
            before=before,success=success,after=after,after_script=after_script,events=events,history=history,
            command_active=active,undo=undo,redo=redo,groups_before=groups_before,groups_after=groups_after,
            groups_undo=groups_undo,groups_redo=groups_redo,tolerance=1e-6,history_cleared_before_command=True),0
    finally:
        if Rhino.Commands.Command.InCommand(): Rhino.RhinoApp.RunScript('!',False)
        doc.Objects.UnselectAll()
        for obj in list(doc.Objects): doc.Objects.Delete(obj.Id,True)
        doc.Layers.SetCurrentLayerIndex(saved_layer,True)
        for i in range(doc.Groups.Count):
            if i not in baseline_groups and not doc.Groups.IsDeleted(i): doc.Groups.Delete(i)
        for i in reversed(layers): doc.Layers.Delete(i,True)
        viewport.SetConstructionPlane(saved_plane)
        doc.ModelAbsoluteTolerance = saved_tolerance
        for g in reversed(owned): g.Dispose()
