"""One prescribed Move/Copy destination click, with public camera calibration."""
import os


def validate(recipe):
    if __package__:
        from .transform_copy_probe import finite
    else:
        from transform_copy_probe import finite
    point = lambda p: isinstance(p, list) and len(p) == 3 and all(finite(v) for v in p)
    if (not isinstance(recipe, dict) or set(recipe) != set(('view', 'bounds', 'aim', 'offset'))
            or recipe['view'] not in ('Front', 'Right', 'Perspective')
            or not point(recipe['aim'])
            or not isinstance(recipe['bounds'], list) or len(recipe['bounds']) != 2
            or not all(point(p) for p in recipe['bounds'])
            or any(a > b for a, b in zip(*recipe['bounds'])) or recipe['bounds'][0] == recipe['bounds'][1]
            or not isinstance(recipe['offset'], list) or len(recipe['offset']) != 2
            or any(type(v) is not int or not -32 <= v <= 32 for v in recipe['offset'])):
        raise ValueError('invalid owned translation click')



def validate_frame(frame):
    if __package__:
        from .transform_copy_probe import finite
    else:
        from transform_copy_probe import finite
    point = lambda p: isinstance(p,list) and len(p) == 3 and all(finite(v) for v in p)
    if (not isinstance(frame,dict) or set(frame) != set(('world_to_screen','aim','aim_client','click_client','size','camera_location','camera_direction','perspective','ray'))
            or any(not point(frame[key]) for key in ('aim','camera_location','camera_direction'))
            or not any(frame['camera_direction']) or type(frame['perspective']) is not bool
            or not isinstance(frame['world_to_screen'],list) or len(frame['world_to_screen']) != 4
            or any(not isinstance(row,list) or len(row) != 4 or not all(finite(v) for v in row) for row in frame['world_to_screen'])
            or not isinstance(frame['ray'],list) or len(frame['ray']) != 2 or not all(point(p) for p in frame['ray']) or frame['ray'][0] == frame['ray'][1]
            or any(not isinstance(frame[key],list) or len(frame[key]) != 2 for key in ('aim_client','click_client','size'))
            or not all(finite(v) for v in frame['aim_client'])
            or any(type(v) is not int for key in ('click_client','size') for v in frame[key])
            or any(not 1 <= pixel < size-1 for pixel,size in zip(frame['click_client'],frame['size']))):
        raise ValueError('invalid translation camera calibration')

def drive(operation, host, script, frames):
    import clr
    clr.AddReference('System.Windows.Forms')
    from System.Windows.Forms import Timer
    from shrink_face_input import _input_hooks
    from snap_environment import environment
    from viewport_capture import capture
    Rhino, System = host['Rhino'], host['System']
    doc = Rhino.RhinoDoc.ActiveDoc
    recipe = operation['mouse_target']
    view = doc.Views.ActiveView
    viewport = view.ActiveViewport
    original = Rhino.DocObjects.ViewportInfo(viewport)
    original_name = viewport.Name
    plane = viewport.ConstructionPlane()
    timer = Timer()
    active, clicked, errors = [], [], []
    progress = os.path.join(os.path.dirname(os.path.abspath(host['__file__'])), 'worker-progress.log')
    def send(line):
        with open(progress, 'a') as stream:
            stream.write(line+'\n'); stream.flush()
    def begun(sender, event):
        if event.CommandEnglishName == operation['command']: active.append(True)
    def ended(sender, event):
        if event.CommandEnglishName == operation['command']: active[:] = []
    class MouseEvents(Rhino.UI.MouseCallback):
        def OnMouseDown(self, event):
            try:
                if (not active or not Rhino.Input.RhinoGet.InGetPoint(doc)
                        or event.View.ActiveViewport.Id != viewport.Id):
                    raise ValueError('translation click outside destination prompt')
                x,y = int(event.ViewportPoint.X), int(event.ViewportPoint.Y)
                clicked.append([x,y])
                frame = capture(viewport, host['_point'](recipe['aim']), [x,y], host)
                ok, ray = viewport.GetFrustumLine(x,y)
                if not ok: raise ValueError('translation pick ray unavailable')
                frame['ray'] = [host['_xyz'](ray.From), host['_xyz'](ray.To)]
                frames.append(frame)
            except Exception as error:
                errors.append(str(error)); send('PICK_ABORT '+operation['id'])
    mouse = MouseEvents()
    try:
        Rhino.RhinoApp.RunScript('_SetView _World _'+recipe['view'], False)
        viewport.SetConstructionPlane(plane)
        viewport.ZoomBoundingBox(Rhino.Geometry.BoundingBox(*[host['_point'](p) for p in recipe['bounds']]))
        doc.Views.Redraw()
        aim = host['_point'](recipe['aim'])
        pixel = viewport.WorldToClient(aim)
        x,y = int(pixel.X)+recipe['offset'][0], int(pixel.Y)+recipe['offset'][1]
        if not 1 <= x < viewport.Size.Width-1 or not 1 <= y < viewport.Size.Height-1:
            raise ValueError('translation click outside owned viewport')
        screen = view.ClientToScreen(System.Drawing.Point(x,y))
        with environment({}, host), _input_hooks(timer, mouse,
                [(Rhino.Commands.Command.BeginCommand,begun),(Rhino.Commands.Command.EndCommand,ended)]):
            # The host waits 0.5 seconds before delivery. Observe the actual
            # destination GetPoint and its camera in OnMouseDown; a mistimed
            # input fails capture instead of manufacturing a result.
            send('PICK @translation:'+operation['id']+' %d %d' % (screen.X,screen.Y))
            result = Rhino.RhinoApp.RunScript(script, True)
        if errors or len(frames) != 1 or clicked != [[x,y]]:
            raise ValueError('incomplete translation click: %s %s' % (errors,clicked))
        return result
    finally:
        viewport.SetViewProjection(original, False)
        viewport.Name = original_name
        original.Dispose()
        doc.Views.Redraw()
