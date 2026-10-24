"""Physical trim-region picks confined to an owned private Rhino viewport."""


def drive(case, ids, host, snapshot):
    import os
    R, G = host['Rhino'], host['Rhino'].Geometry
    doc = R.RhinoDoc.ActiveDoc
    view = doc.Views.ActiveView
    viewport = view.ActiveViewport
    host['_record_progress']('trim input: configure projection')
    projection = (R.Display.DefinedViewportProjection.Perspective if 'camera' in case
                  else R.Display.DefinedViewportProjection.Top)
    if not viewport.SetProjection(projection, 'Owned MeshTrim input', False):
        raise ValueError('owned trim viewport unavailable')
    if 'camera' in case:
        camera = case['camera']
        viewport.SetCameraLocations(G.Point3d(*camera['target']), G.Point3d(*camera['location']))
    mode = R.Display.DisplayModeDescription.FindByName('Shaded')
    if mode is None:
        raise ValueError('owned shaded mode unavailable')
    host['_record_progress']('trim input: configure display')
    viewport.DisplayMode = mode
    bounds = doc.Objects.FindId(ids[0]).Geometry.GetBoundingBox(True)
    if case.get('fit_sources'):
        for identifier in ids[1:]:
            bounds.Union(doc.Objects.FindId(identifier).Geometry.GetBoundingBox(True))
    bounds.Inflate(1.)
    host['_record_progress']('trim input: fit target')
    viewport.ZoomBoundingBox(bounds)
    host['_record_progress']('trim input: redraw')
    doc.Views.Redraw()
    host['_record_progress']('trim input: prepare sequence')
    points = [[G.Point3d(*p)] for p in case['trim_picks']]
    split_control = None
    if 'sdk_cutters' in case:
        from mesh_trim_split_control import capture
        split_control = capture(case, ids, host)
    script = '_-MeshTrim ' + case.get('arguments', '')
    if not case['preselect']:
        script += ' ' + ' '.join('_SelID ' + str(ids[i]) for i in case['first'])
        script += ' ' + case.get('cutting_arguments', '') + ' _Enter'
    direct = case.get('direct_getter', True)
    if not direct:
        script += ' _Pause' * len(points) + ' _' + case.get('finish', 'Enter')
    progress = os.path.join(os.path.dirname(os.path.abspath(host['__file__'])), 'worker-progress.log')
    from mesh_trim_pick_trace import observing_picks
    with observing_picks(host, view, len(points)+4, case.get('trace_picks', False)) as trace:
        for index,values in enumerate(points):
            pixel = viewport.WorldToClient(values[0])
            if not 1 <= pixel.X < viewport.Size.Width-1 or not 1 <= pixel.Y < viewport.Size.Height-1:
                raise ValueError('trim pick outside owned viewport')
            screen = view.ClientToScreen(host['System'].Drawing.Point(int(pixel.X), int(pixel.Y)))
            if trace is not None:
                trace.query(values[0], int(pixel.X), int(pixel.Y), ids, case.get('trace_objects', False))
            with open(progress, 'a') as stream:
                stream.write('PICK @component-click:%s:%d:plain %d %d\n' % (case['capture_id'],index,screen.X,screen.Y))
                stream.flush()
        host['_record_progress']('trim input: run physical selection')
        if direct:
            with open(progress, 'a') as stream:
                stream.write('PICK @component-finish:%s:%s 1 1\n' % (case['capture_id'], case.get('finish', 'Enter')))
                stream.flush()
        result = R.RhinoApp.RunScript(script, True)
    xyz = lambda p: [float(p.X), float(p.Y), float(p.Z)]
    cplane = viewport.GetConstructionPlane().Plane
    extra = dict(input_script=script, trim_view=dict(
        location=xyz(viewport.CameraLocation), direction=xyz(viewport.CameraDirection),
        up=xyz(viewport.CameraUp), target=xyz(viewport.CameraTarget),
        perspective=bool(viewport.IsPerspectiveProjection),
        cplane=[xyz(cplane.Origin), xyz(cplane.XAxis), xyz(cplane.YAxis)]))
    if trace is not None:
        extra['trim_pick_trace'] = trace.result()
    if split_control is not None:
        extra['trim_split_control'] = split_control
    return result, extra
