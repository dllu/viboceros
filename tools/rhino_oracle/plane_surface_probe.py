"""Capture public Plane surfaces and atomic document behavior in an owned job."""
if __package__:
    from .plane_surface_cases import validate, recipe
else:
    from plane_surface_cases import validate, recipe


def run(op, host, iterations, tolerance):
    validate(op)
    from join_probe import observe_command
    R, S = host['Rhino'], host['System']
    G = R.Geometry
    doc = R.RhinoDoc.ActiveDoc
    if iterations != 1 or list(doc.Objects) or R.Commands.Command.InCommand():
        raise ValueError('empty owned idle document required')
    vp = doc.Views.ActiveView.ActiveViewport
    old = (doc.Layers.CurrentLayerIndex, vp.ConstructionPlane(),
           doc.ModelAbsoluteTolerance, doc.ModelRelativeTolerance, doc.ModelAngleToleranceRadians)
    layers, source, guide, group = [], None, None, None
    case = recipe(op['case'])
    original_view = R.DocObjects.ViewportInfo(vp) if case.get('view') else None

    def snapshot():
        rows = []
        for obj in sorted(list(doc.Objects), key=lambda o: int(o.RuntimeSerialNumber)):
            a, g = obj.Attributes, obj.Geometry
            row = dict(source=obj.Id == source, guide=obj.Id == guide, selected=int(obj.IsSelected(False)),
                       native_type=g.GetType().Name, native_valid=bool(g.IsValid),
                       attributes=dict(name=a.Name or '', layer=doc.Layers[a.LayerIndex].Name,
                                       color_source=str(a.ColorSource),
                                       color=[int(a.ObjectColor.R), int(a.ObjectColor.G), int(a.ObjectColor.B)],
                                       groups=[doc.Groups.GroupName(i) for i in (a.GetGroupList() or [])]))
            if obj.Id == source:
                row['point'] = host['_xyz'](g.Location)
            elif obj.Id == guide:
                row['curve'] = host['_nurbs_curve_definition'](g)
            elif isinstance(g, G.Brep):
                row['surfaces'] = [host['_nurbs_surface_definition'](f.UnderlyingSurface()) for f in g.Faces]
                row['face_count'] = g.Faces.Count
            elif isinstance(g, G.Surface):
                row['surfaces'] = [host['_nurbs_surface_definition'](g)]
                row['face_count'] = 1
            else:
                raise ValueError('unexpected Plane output: ' + row['native_type'])
            rows.append(row)
        return rows

    try:
        doc.ModelAbsoluteTolerance = tolerance['absolute']
        doc.ModelRelativeTolerance = tolerance['relative']
        doc.ModelAngleToleranceRadians = tolerance['angular']
        frame = case.get('construction_plane', [[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]])
        plane = G.Plane(G.Point3d(*frame[0]), G.Vector3d(*frame[1]), G.Vector3d(*frame[2]))
        if not plane.IsValid:
            raise ValueError('invalid Plane frame')
        if case.get('view'):
            vp.SetProjection(getattr(R.Display.DefinedViewportProjection,case['view']),'Owned Plane completion view',False)
        vp.SetConstructionPlane(plane)
        if case.get('pointer_pixel'):
            import clr
            clr.AddReference('System.Windows.Forms')
            from System.Windows.Forms import Cursor
            u, v = case['pointer_pixel']
            pixel = S.Drawing.Point(int(u * vp.Size.Width),int(v * vp.Size.Height))
            Cursor.Position = doc.Views.ActiveView.ClientToScreen(pixel)
        for name in ['Plane sources', 'Plane output']:
            layer = R.DocObjects.Layer(); layer.Name = name
            layers.append(doc.Layers.Add(layer)); layer.Dispose()
        a = R.DocObjects.ObjectAttributes(); a.Name = 'sentinel'; a.LayerIndex = layers[0]
        a.ObjectColor = S.Drawing.Color.FromArgb(12, 34, 56)
        a.ColorSource = R.DocObjects.ObjectColorSource.ColorFromObject
        source = doc.Objects.AddPoint(G.Point3d(50., 60., 70.), a); a.Dispose()
        if 'guide' in case:
            a = R.DocObjects.ObjectAttributes(); a.Name = 'guide'; a.LayerIndex = layers[0]
            g = G.LineCurve(G.Point3d(*case['guide'][0]), G.Point3d(*case['guide'][1]))
            guide = doc.Objects.AddCurve(g, a); g.Dispose(); a.Dispose()
        group = doc.Groups.Add('Plane source group', [source] + ([guide] if guide is not None else []))
        if case.get('selected', True): doc.Objects.Select(source)
        if case.get('guide_preselected'): doc.Objects.Select(guide)
        doc.Layers.SetCurrentLayerIndex(layers[1], True)
        doc.ClearUndoRecords(True)
        before = snapshot()
        def token(a):
            return '_' + a if isinstance(a, str) else 'w' + ','.join('%.17g' % n for n in a) if isinstance(a, list) else '%.17g' % a
        words = []
        for a in case['actions']:
            words.append(token(a))
            if a == 'AroundCurve' and not case.get('skip_guide_pick'): words.extend(["'_-SelID", str(guide)])
        macro = '_Plane ' + ' '.join(words)
        marker = 'Owned Plane ' + str(S.Guid.NewGuid())
        R.RhinoApp.WriteLine(marker)
        pointer = None
        if case.get('pointer'):
            from plane_finish_pointer import record
            pointer = record(host,vp)
        traces = []
        def invoke():
            if not case.get('paced'): return R.RhinoApp.RunScript(macro, True)
            from conic_getter_input import drive
            actions = []
            for a in case['actions']:
                actions.append(a)
                if a == 'AroundCurve' and not case.get('skip_guide_pick'):
                    actions.append(dict(curve=0))
            returned, trace = drive(dict(id=op['id'], actions=actions), host,
                                    [guide] if guide is not None else [], 'Plane')
            traces.append(trace)
            return returned
        success, created, events = observe_command(R.Commands.Command, 'Plane',
            invoke, snapshot, lambda: [], True)
        after = snapshot()
        pointer_after = record(host,vp) if pointer is not None else None
        history = R.RhinoApp.CommandHistoryWindowText.split(marker, 1)[1].strip()
        if R.Commands.Command.InCommand():
            raise ValueError('Plane did not return to idle: ' + history)
        after_plane = vp.ConstructionPlane()
        if success:
            R.RhinoApp.RunScript('_Undo', False); undo = snapshot()
            R.RhinoApp.RunScript('_Redo', False); redo = snapshot()
        else:
            undo, redo = before, after
        return dict(success=success, before=before, created=created, after=after,
                    undo=undo, redo=redo, history=history,
                    macro='_Plane _Pause' if case.get('paced') else macro,
                    planned_macro=macro, events=events,
                    construction_plane=frame, returned_idle=True,
                    input_trace=traces,
                    pointer=pointer,
                    pointer_after=pointer_after,
                    model_tolerance=dict(absolute=doc.ModelAbsoluteTolerance,
                                         relative=doc.ModelRelativeTolerance,
                                         angular=doc.ModelAngleToleranceRadians),
                    cplane_after=[host['_xyz'](after_plane.Origin), host['_xyz'](after_plane.XAxis), host['_xyz'](after_plane.YAxis)]), 0
    finally:
        R.RhinoApp.RunScript('!', False)
        for obj in list(doc.Objects): doc.Objects.Delete(obj.Id, True)
        doc.ClearUndoRecords(True)
        if group is not None: doc.Groups.Delete(group)
        doc.Layers.SetCurrentLayerIndex(old[0], True)
        if original_view is not None:
            vp.SetViewProjection(original_view,False);original_view.Dispose()
        vp.SetConstructionPlane(old[1])
        doc.ModelAbsoluteTolerance, doc.ModelRelativeTolerance, doc.ModelAngleToleranceRadians = old[2:]
        for index in reversed(layers): doc.Layers.Delete(index, True)
