"""Named mixed ShowEdges/ZoomNaked commands with immutable shared operands."""
if __package__:
    from .edge_analysis_mixed_cases import validate
else:
    from edge_analysis_mixed_cases import validate


def geometry(g, host):
    G = host['Rhino'].Geometry
    if isinstance(g, G.Brep):
        return dict(kind='brep', value=host['_interchange_brep_record'](g))
    if isinstance(g, G.SubD):
        from subd_direction_probe import geometry as subd_geometry
        return subd_geometry(g, host)
    if isinstance(g, G.Mesh):
        return dict(kind='mesh', points=[host['_xyz'](v) for v in g.Vertices.ToPoint3dArray()],
                    faces=[[int(f.A), int(f.B), int(f.C)]+([] if f.IsTriangle else [int(f.D)]) for f in g.Faces])
    if isinstance(g, G.Point):
        return dict(kind='point', point=host['_xyz'](g.Location))
    raise ValueError('owned mixed source type required')


def source(kind, host):
    R = host['Rhino']
    G = R.Geometry
    if kind == 'brep':
        import os
        path = os.path.join(os.path.dirname(__file__), 'fixtures', 'brep_direction', 'warped.3dm')
        model = R.FileIO.File3dm.Read(path)
        try:
            return list(model.Objects)[0].Geometry.DuplicateBrep()
        finally:
            model.Dispose()
    if kind == 'subd':
        from subd_edge_analysis_probe import source as subd_source
        return subd_source('open_cube', host)
    if kind == 'point':
        return G.Point(G.Point3d(30, 30, 0))
    mesh = G.Mesh()
    mesh.Vertices.UseDoublePrecisionVertices = True
    for p in [(20, 0, 0), (24, 0, 0), (24, 3, 1), (20, 3, 0)]:
        mesh.Vertices.Add(G.Point3d(*p))
    mesh.Faces.AddFace(0, 1, 2, 3)
    return mesh


def run(op, host, iterations, tolerance):
    validate(op)
    from join_probe import observe_command
    R, S = host['Rhino'], host['System']
    doc = R.RhinoDoc.ActiveDoc
    if iterations != 1 or list(doc.Objects) or R.Commands.Command.InCommand():
        raise ValueError('empty owned idle document required')
    old = doc.SubDAppearance
    keys, kinds = [], op['order']+(['point'] if op['extra_point'] else [])
    group = -1
    finish_input = None
    try:
        for kind in kinds:
            g = source(kind, host)
            attrs = R.DocObjects.ObjectAttributes()
            attrs.Name = kind
            try:
                key = doc.Objects.Add(g, attrs)
            finally:
                attrs.Dispose()
                g.Dispose()
            if key == S.Guid.Empty:
                raise ValueError('owned source admission failed')
            keys.append(key)
        if op['grouped']:
            group = doc.Groups.Add('Owned diagnostics', keys)
            if group < 0:
                raise ValueError('owned diagnostic group was not created')
        doc.SubDAppearance = R.Geometry.SubDComponentLocation.ControlNet if op['flat'] else R.Geometry.SubDComponentLocation.Surface
        model = R.FileIO.File3dm()
        try:
            for key in keys:
                g=doc.Objects.FindId(key).Geometry
                if isinstance(g,R.Geometry.Brep):model.Objects.AddBrep(g)
                elif isinstance(g,R.Geometry.Mesh):model.Objects.AddMesh(g)
                elif isinstance(g,R.Geometry.SubD):model.Objects.AddSubD(g)
                else:model.Objects.AddPoint(g.Location)
            archive = S.Convert.ToBase64String(model.ToByteArray())
        finally:
            model.Dispose()

        def snapshot():
            rows = []
            for kind, key in zip(kinds, keys):
                obj = doc.Objects.FindId(key)
                rows.append(dict(kind=kind, name=str(obj.Attributes.Name), groups=int(obj.Attributes.GroupCount),
                                 geometry=geometry(obj.Geometry, host), selected=bool(obj.IsSelected(False))))
            points = [o for o in doc.Objects if o.Id not in keys and isinstance(o.Geometry, R.Geometry.Point)]
            points.sort(key=lambda o: int(o.RuntimeSerialNumber))
            return dict(sources=rows, objects=len(list(doc.Objects)), points=[host['_xyz'](o.Geometry.Location) for o in points])

        R.RhinoApp.RunScript('_ShowEdgesOff', False)
        doc.Objects.UnselectAll()
        doc.ClearUndoRecords(True)
        R.RhinoApp.RunScript('_Point w40,50,60', False)
        R.RhinoApp.RunScript('_Undo', False)
        if op['preselect']:
            for key in (keys[:1] if op.get('membership') and not op.get('panel_only') else keys):
                doc.Objects.Select(key)
        before = snapshot()
        marker = 'Owned mixed edge analysis ' + str(S.Guid.NewGuid())
        R.RhinoApp.WriteLine(marker)
        select = '' if op['preselect'] else ' '+' '.join('_SelID '+str(key) for key in keys)+' _Enter'
        macro = '_'+op['command']+select
        if op['command'] != 'ShowEdges':
            macro += ' '+' '.join('_'+step for step in op['actions'])+' _Enter'
        success, event_state, events = observe_command(R.Commands.Command, op['command'], lambda: R.RhinoApp.RunScript(macro, True), snapshot, lambda: [], True)
        R.RhinoApp.Wait()
        stages = []
        if op.get('membership'):
            from owned_panel import inspect
            stages.append(dict(action='Start',state=snapshot()))
            if not op.get('panel_only'):
                batches = [[key] for key in keys[1:]] if op.get('sequential') else [keys[1:]]
                for i, batch in enumerate(batches):
                    suffix = str(i+1) if op.get('sequential') else ''
                    doc.Objects.UnselectAll()
                    for key in batch:doc.Objects.Select(key)
                    stages.append(dict(action='Select'+suffix if suffix else 'SelectRest',state=snapshot()))
                    inspect('Edge Analysis','Add Objects');R.RhinoApp.Wait()
                    stages.append(dict(action='Add'+suffix,state=snapshot()))
                doc.Objects.UnselectAll();doc.Objects.Select(keys[0])
                stages.append(dict(action='SelectFirst',state=snapshot()))
                inspect('Edge Analysis','Remove Objects');R.RhinoApp.Wait()
                stages.append(dict(action='Remove',state=snapshot()))
            inspect('Edge Analysis','Naked edges');R.RhinoApp.Wait()
            from edge_zoom_input import start
            finish_input=start(R,S,['_All','_Mark',''])
            inspect('Edge Analysis','Zoom');R.RhinoApp.Wait()
            stages.append(dict(action='Zoom',input=finish_input(),state=snapshot()))
        after = snapshot()
        if after['points']:
            R.RhinoApp.RunScript('_Undo', False)
            undo = snapshot()
            R.RhinoApp.RunScript('_Redo', False)
            redo = snapshot()
        else:
            R.RhinoApp.RunScript('_Redo', False)
            redo = snapshot()
            R.RhinoApp.RunScript('_Undo', False)
            undo = snapshot()
        return dict(source_archive=archive, stages=stages, before=before, event_state=event_state, after=after, undo=undo, redo=redo,
                    success=success, events=events, macro=macro,
                    history=R.RhinoApp.CommandHistoryWindowText.split(marker, 1)[1].strip()), 0
    finally:
        if finish_input is not None:finish_input.stop()
        R.RhinoApp.RunScript('!', False)
        R.RhinoApp.RunScript('_ShowEdgesOff', False)
        for obj in list(doc.Objects):
            doc.Objects.Delete(obj.Id, True)
        if group >= 0:
            doc.Groups.Delete(group)
        doc.SubDAppearance = old
