"""Bounded native Mirror cursor previews in an empty, owned document."""
import json
import os
import re


def validate_request(request):
    operations = request.get('operations')
    if (type(request.get('protocol_version')) is not int or request['protocol_version'] != 1
            or type(request.get('iterations', 1)) is not int or request.get('iterations', 1) != 1
            or not isinstance(operations, list) or not 1 <= len(operations) <= 24):
        raise ValueError('Mirror preview requires protocol 1 and one iteration')
    names = set()
    for op in operations:
        if (not isinstance(op, dict) or set(op) - {'cursor'} != {'op', 'id', 'copy', 'plane', 'display_mode', 'finish'}):
            raise ValueError('invalid Mirror preview fields')
        if (op['op'] != 'mirror_preview' or not isinstance(op['id'], (str, type(u'')))
                or re.match(r'^[A-Za-z0-9_-]{1,80}\Z', op['id']) is None or op['id'] in names
                or type(op['copy']) is not bool or op['plane'] not in ('TwoPoint', 'ThreePoint')
                or op['display_mode'] not in ('Wireframe', 'Shaded', 'Ghosted')
                or op['finish'] not in ('Click', 'Cancel')
                or op.get('cursor', 'Valid') not in ('Valid', 'Degenerate')
                or (op.get('cursor') == 'Degenerate' and op['finish'] != 'Cancel')):
            raise ValueError('invalid Mirror preview case')
        names.add(op['id'])


def run(operation, host):
    validate_request({'protocol_version': 1, 'operations': [operation]})
    Rhino, System = host['Rhino'], host['System']
    import clr
    clr.AddReference('System.Windows.Forms')
    from System.Windows.Forms import Timer
    document = Rhino.RhinoDoc.ActiveDoc
    if list(document.Objects):
        raise ValueError('Mirror preview requires an empty owned document')
    view = document.Views.ActiveView
    viewport = view.ActiveViewport
    original_projection = Rhino.DocObjects.ViewportInfo(viewport)
    original_name, original_target = viewport.Name, viewport.CameraTarget
    original_mode = viewport.DisplayMode
    aids = Rhino.ApplicationSettings.ModelAidSettings
    track = Rhino.ApplicationSettings.SmartTrackSettings
    original_aids, original_track = aids.GetCurrentState(), track.GetCurrentState()
    root = os.path.dirname(os.path.abspath(host['__file__']))
    progress = os.path.join(root, 'worker-progress.log')
    captured_path = os.path.join(root, 'mirror-preview-captured-%s.json' % operation['id'])
    ready_path = os.path.join(root, 'mirror-preview-ready-%s.json' % operation['id'])
    sources, pending, errors = [], [], []
    timer = Timer()
    timer.Interval = 100

    def snapshot():
        result = []
        for obj in document.Objects:
            bounds = obj.Geometry.GetBoundingBox(True)
            result.append(dict(id=str(obj.Id), selected=bool(obj.IsSelected(False)),
                               type=str(obj.Geometry.ObjectType),
                               bounds=[host['_xyz'](bounds.Min), host['_xyz'](bounds.Max)]))
        return sorted(result, key=lambda item: item['id'])

    def tick(sender, event):
        try:
            if pending or errors or not os.path.exists(captured_path):
                return
            with open(captured_path) as stream:
                if json.load(stream) != operation['id']:
                    raise ValueError('foreign preview capture acknowledgement')
            prompt = Rhino.RhinoApp.CommandPrompt
            expected = 'End of mirror plane' if operation['plane'] == 'TwoPoint' else 'Third point of mirror plane'
            if not prompt.startswith(expected):
                raise ValueError('preview capture was not at the final Mirror point: %s' % prompt)
            pending[:] = [snapshot()]
            timer.Stop()
            with open(ready_path + '.tmp', 'w') as stream:
                json.dump(operation['id'], stream)
            os.rename(ready_path + '.tmp', ready_path)
        except Exception as error:
            errors.append(str(error))
            with open(progress, 'a') as stream:
                stream.write('PICK_ABORT mirror-preview-%s\n' % operation['id'])
                stream.flush()

    try:
        aids.Ortho = aids.GridSnap = aids.Osnap = False
        track.UseSmartTrack = False
        if not viewport.SetProjection(Rhino.Display.DefinedViewportProjection.Top, 'Mirror preview', False):
            raise ValueError('could not set owned Top projection')
        mode = Rhino.Display.DisplayModeDescription.FindByName(operation['display_mode'])
        if mode is None:
            raise ValueError('native display mode unavailable')
        viewport.DisplayMode = mode
        bounds = Rhino.Geometry.BoundingBox(host['_point']([-8, -7, -1]), host['_point']([8, 7, 4]))
        if not viewport.ZoomBoundingBox(bounds):
            raise ValueError('could not fit owned preview viewport')
        attributes = Rhino.DocObjects.ObjectAttributes()
        attributes.ColorSource = Rhino.DocObjects.ObjectColorSource.ColorFromObject
        attributes.ObjectColor = System.Drawing.Color.FromArgb(200, 80, 60)
        sources.append(document.Objects.AddPoint(host['_point']([3, 3, 0]), attributes))
        curve = Rhino.Geometry.PolylineCurve([host['_point'](p) for p in
                                             ([2, 1, 0], [4, 1, 0], [3, -1, 0], [2, 1, 0])])
        sources.append(document.Objects.AddCurve(curve, attributes))
        box = Rhino.Geometry.BoundingBox(host['_point']([2, -5, 0]), host['_point']([4, -3, 2]))
        sources.append(document.Objects.AddBrep(Rhino.Geometry.Brep.CreateFromBox(box), attributes))
        for key in sources:
            if key == System.Guid.Empty:
                raise ValueError('could not create owned preview source')
            document.Objects.Select(key)
        document.Views.Redraw()
        pixel = viewport.WorldToClient(host['_point']([0, 0 if operation.get('cursor') == 'Degenerate' else 5, 0]))
        screen = view.ClientToScreen(System.Drawing.Point(int(pixel.X), int(pixel.Y)))
        valid_pixel = viewport.WorldToClient(host['_point']([0, 5, 0]))
        valid_screen = view.ClientToScreen(System.Drawing.Point(int(valid_pixel.X), int(valid_pixel.Y)))
        corner = view.ClientToScreen(System.Drawing.Point(0, 0))
        metadata = dict(rect=[int(corner.X), int(corner.Y),
                              int(corner.X + viewport.Size.Width), int(corner.Y + viewport.Size.Height)],
                        aim_client=[float(pixel.X), float(pixel.Y)],
                        valid_screen=[int(valid_screen.X), int(valid_screen.Y)],
                        regions={name: [float(viewport.WorldToClient(host['_point'](p)).X),
                                        float(viewport.WorldToClient(host['_point'](p)).Y)]
                                 for name, p in [('source_point', [3, 3, 0]), ('reflected_point', [-3, 3, 0]),
                                                 ('source_box', [3, -4, 2]), ('reflected_box', [-3, -4, 2])]})
        with open(os.path.join(root, 'mirror-preview-%s.json' % operation['id']), 'w') as stream:
            json.dump(metadata, stream)
        before = snapshot()
        start = Rhino.RhinoApp.CommandHistoryWindowText
        timer.Tick += tick
        timer.Start()
        prefix = '_Mirror _Copy=%s ' % ('_Yes' if operation['copy'] else '_No')
        plane = 'w0,0,0 ' if operation['plane'] == 'TwoPoint' else '_3Point w0,0,0 w0,0,3 '
        with open(progress, 'a') as stream:
            stream.write('PICK @mirror-preview:%s %d %d\n' % (operation['id'], screen.X, screen.Y))
            stream.flush()
        success = bool(Rhino.RhinoApp.RunScript(prefix + plane + '_Pause', False))
        timer.Stop()
        history = Rhino.RhinoApp.CommandHistoryWindowText
        history = history[len(start):] if history.startswith(start) else history[-2000:]
        if errors or not pending:
            raise ValueError('Mirror preview did not reach owned cursor: %s' % errors)
        return dict(before=before, pending=pending[0], after=snapshot(), success=success,
                    history=history[-2000:], calibration=metadata), 0
    finally:
        timer.Stop()
        timer.Tick -= tick
        timer.Dispose()
        for obj in list(document.Objects):
            document.Objects.Delete(obj.Id, True)
        viewport.SetViewProjection(original_projection, False)
        viewport.SetCameraTarget(original_target, False)
        viewport.Name = original_name
        viewport.DisplayMode = original_mode
        aids.UpdateFromState(original_aids)
        track.UpdateFromState(original_track)
