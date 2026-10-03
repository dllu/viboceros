"""Owned Move Normal references and calibrated native reference clicks."""
import os


def validate_camera(camera, frame):
    if __package__:
        from .transform_copy_probe import finite
    else:
        from transform_copy_probe import finite
    vectors = ('camera_location','camera_target','camera_direction','camera_up','cplane_origin','cplane_x','cplane_y')
    keys = set(vectors) | {'perspective','two_point_perspective','frustum','viewport_size','projected_points'}
    point = lambda p: isinstance(p,list) and len(p) == 3 and all(finite(v) for v in p)
    if (not isinstance(camera,dict) or set(camera) != keys
            or any(not point(camera[key]) for key in vectors)
            or any(type(camera[key]) is not bool for key in ('perspective','two_point_perspective'))
            or camera['viewport_size'] != frame['size'] or camera['perspective'] != frame['perspective']
            or camera['camera_location'] != frame['camera_location'] or camera['camera_direction'] != frame['camera_direction']
            or not isinstance(camera['frustum'],list) or len(camera['frustum']) != 6
            or not all(finite(v) for v in camera['frustum'])
            or any(camera['frustum'][a] >= camera['frustum'][b] for a,b in ((0,1),(2,3),(4,5)))
            or not isinstance(camera['projected_points'],list) or len(camera['projected_points']) != 3
            or any(not isinstance(p,dict) or set(p) != {'point','screen'} or not point(p['point'])
                   or not isinstance(p['screen'],list) or len(p['screen']) != 2 or not all(finite(v) for v in p['screen']) for p in camera['projected_points'])):
        raise ValueError('invalid Move Normal camera')
    for key in ('camera_direction','camera_up','cplane_x','cplane_y'):
        if abs(sum(v*v for v in camera[key])-1) > 1e-9: raise ValueError('invalid Move Normal camera axis')
    for a,b in (('camera_direction','camera_up'),('cplane_x','cplane_y')):
        if abs(sum(x*y for x,y in zip(camera[a],camera[b]))) > 1e-9: raise ValueError('invalid Move Normal camera frame')


def validate_target(target):
    if __package__:
        from .transform_copy_probe import finite
    else:
        from transform_copy_probe import finite
    point = lambda p: isinstance(p,list) and len(p) == 3 and all(finite(v) and abs(v) <= 1e6 for v in p)
    if (not isinstance(target,dict) or set(target) != set(('kind','points','aim','view','flip'))
            or target['kind'] not in ('line','circle','arc','polyline','surface','brep','trimmed')
            or target['view'] not in ('Top','Front','Right','Perspective')
            or type(target['flip']) is not bool or not point(target['aim'])
            or not isinstance(target['points'],list) or not all(point(p) for p in target['points'])):
        raise ValueError('invalid owned Move Normal reference')
    count = dict(line=2,circle=1,arc=3,polyline=3,surface=4,brep=4,trimmed=4)[target['kind']]
    if len(target['points']) != count: raise ValueError('invalid Move Normal reference control count')


def geometry(target, host):
    Rhino = host['Rhino']; p = [host['_point'](v) for v in target['points']]
    kind = target['kind']
    if kind == 'line': result = Rhino.Geometry.LineCurve(p[0],p[1])
    elif kind == 'circle': result = Rhino.Geometry.Circle(p[0],3.).ToNurbsCurve()
    elif kind == 'arc': result = Rhino.Geometry.Arc(p[0],p[1],p[2]).ToNurbsCurve()
    elif kind == 'polyline': result = Rhino.Geometry.PolylineCurve(p)
    else:
        result = Rhino.Geometry.NurbsSurface.CreateFromCorners(*p)
        if kind in ('brep','trimmed'):
            surface = result
            try: result = surface.ToBrep()
            finally: surface.Dispose()
        if kind == 'trimmed':
            original = result
            cut = original.Faces[0].IsoCurve(1,original.Faces[0].Domain(0).Mid)
            split = None
            try:
                split = original.Faces[0].Split([cut],1e-9)
                if split is None or split.Faces.Count != 2: raise ValueError('Move Normal trim split failed')
                # Retain the left parameter half and its original underlying
                # surface. Shrinking would destroy IgnoreTrims coverage.
                face = min(list(split.Faces),key=lambda f:f.GetBoundingBox(True).Center.X)
                result = face.DuplicateFace(False)
            finally:
                cut.Dispose(); original.Dispose()
                if split is not None: split.Dispose()
    if target['flip']:
        if isinstance(result,Rhino.Geometry.Curve): result.Reverse()
        elif isinstance(result,Rhino.Geometry.Brep): result.Flip()
        else:
            original = result; result = original.Reverse(0)
            original.Dispose()
    return result


def run(operation, host, action):
    from mirror_object_probe import target_snapshot
    target = operation['normal_target']; validate_target(target)
    Rhino, System = host['Rhino'], host['System']; doc = Rhino.RhinoDoc.ActiveDoc
    owned = geometry(target,host); key = System.Guid.Empty
    try:
        if owned is None or not owned.IsValid: raise ValueError('invalid Move Normal geometry')
        key = doc.Objects.Add(owned)
        if key == System.Guid.Empty: raise ValueError('Move Normal reference insertion failed')
        before = target_snapshot(key,host)
        value, elapsed = action(operation,host,dict(target,id=key))
        value['target'] = dict(before=before,after=target_snapshot(key,host))
        return value,elapsed
    finally:
        if key != System.Guid.Empty and not doc.Objects.Delete(key,True): raise ValueError('Move Normal reference cleanup failed')
        owned.Dispose()


def click_reference(operation, host, script, frames):
    import clr
    clr.AddReference('System.Windows.Forms')
    from System.Windows.Forms import Timer
    from shrink_face_input import _input_hooks
    from snap_environment import environment
    from viewport_capture import capture
    from named_view_policy_probe import snapshot as camera_snapshot
    Rhino, System = host['Rhino'], host['System']; doc = Rhino.RhinoDoc.ActiveDoc
    recipe = operation['normal_target']
    view = doc.Views.ActiveView; viewport = view.ActiveViewport
    original = Rhino.DocObjects.ViewportInfo(viewport); name = viewport.Name
    plane = viewport.ConstructionPlane()
    active, clicked, errors = [],[],[]
    progress = os.path.join(os.path.dirname(os.path.abspath(host['__file__'])),'worker-progress.log')
    def send(line):
        with open(progress,'a') as stream: stream.write(line+'\n'); stream.flush()
    def begun(sender,event):
        if event.CommandEnglishName == 'Move': active.append(True)
    def ended(sender,event):
        if event.CommandEnglishName == 'Move': active[:] = []
    class Mouse(Rhino.UI.MouseCallback):
        def OnMouseDown(self,event):
            try:
                if not active or event.View.ActiveViewport.Id != viewport.Id: raise ValueError('Move Normal click outside owned prompt')
                point_phase = Rhino.Input.RhinoGet.InGetPoint(doc)
                object_phase = Rhino.Input.RhinoGet.InGetObject(doc)
                if (not clicked and (not object_phase or point_phase)) or (clicked and not point_phase):
                    raise ValueError('Move Normal click in wrong input phase: point=%s object=%s prompt=%s' % (point_phase,object_phase,Rhino.RhinoApp.CommandPrompt))
                x,y = int(event.ViewportPoint.X),int(event.ViewportPoint.Y)
                clicked.append([x,y]); frame = capture(viewport,host['_point'](recipe['aim']),[x,y],host)
                ok,ray = viewport.GetFrustumLine(x,y)
                if not ok: raise ValueError('Move Normal viewing line unavailable')
                frame['ray'] = [host['_xyz'](ray.From),host['_xyz'](ray.To)]
                frames.append(frame if len(clicked) == 1 else dict(frame=frame,camera=camera_snapshot(viewport,Rhino)))
                host['_record_progress']('Move Normal click point=%s object=%s prompt=%s' % (Rhino.Input.RhinoGet.InGetPoint(doc),Rhino.Input.RhinoGet.InGetObject(doc),Rhino.RhinoApp.CommandPrompt))
            except Exception as error:
                errors.append(str(error)); send('PICK_ABORT '+operation['id'])
    timer,mouse = Timer(),Mouse()
    try:
        Rhino.RhinoApp.RunScript('_SetView _World _'+recipe['view'],False)
        viewport.SetConstructionPlane(plane)
        viewport.ZoomBoundingBox(Rhino.Geometry.BoundingBox(host['_point']([-10,-10,-10]),host['_point']([10,10,10])))
        viewport.DisplayMode = Rhino.Display.DisplayModeDescription.FindByName('Shaded')
        doc.Views.Redraw()
        pixel = viewport.WorldToClient(host['_point'](recipe['aim'])); x,y = int(pixel.X),int(pixel.Y)
        if not 1 <= x < viewport.Size.Width-1 or not 1 <= y < viewport.Size.Height-1: raise ValueError('Move Normal aim outside viewport')
        screen = view.ClientToScreen(System.Drawing.Point(x,y))
        with environment({},host), _input_hooks(timer,mouse,[(Rhino.Commands.Command.BeginCommand,begun),(Rhino.Commands.Command.EndCommand,ended)]):
            send('PICK @move-normal:'+operation['id']+' %d %d' % (screen.X,screen.Y))
            if 'NormalBase' in operation['inputs']:
                send('PICK @move-normal-base:'+operation['id']+' %d %d' % (screen.X,screen.Y))
            result = Rhino.RhinoApp.RunScript(script,True)
        count = 2 if 'NormalBase' in operation['inputs'] else 1
        if errors or clicked != [[x,y]]*count or len(frames) != count: raise ValueError('incomplete Move Normal click %s %s' % (errors,clicked))
        return result
    finally:
        viewport.SetViewProjection(original,False); viewport.Name = name
        original.Dispose(); doc.Views.Redraw()
