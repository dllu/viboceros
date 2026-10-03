"""Owned planar targets for actual Mirror Object commands and face clicks."""
import math


def validate_target(target):
    if not isinstance(target, dict):
        raise ValueError('invalid Mirror target')
    kind = target.get('kind')
    keys = {'kind', 'pick', 'view', 'face', 'corners'}
    if (not keys <= set(target) or not set(target) <= keys | {'display','fraction','modifiers','menu'} or kind not in ('surface', 'trimmed', 'box', 'extrusion', 'curved', 'mesh', 'curve')
            or target['pick'] not in ('id', 'mouse', 'mouse-sub')
            or target['view'] not in ('Top', 'Bottom', 'Front', 'Back', 'Left', 'Right')
            or type(target['face']) is not int or not 0 <= target['face'] <= 5
            or not isinstance(target['corners'], list) or len(target['corners']) != 4
            or any(not isinstance(point, list) or len(point) != 3
                   or any(type(value) not in (int, float) or math.isnan(value) or math.isinf(value)
                          or abs(value) > 1e6 for value in point) for point in target['corners'])):
        raise ValueError('invalid Mirror target')
    if (target.get('menu') not in (None,'First') or target.get('display','Shaded') not in ('Shaded','Wireframe')
            or target.get('modifiers','plain') not in ('plain','sub')
            or not isinstance(target.get('fraction',[.5,.5]),list)
            or len(target.get('fraction',[.5,.5])) != 2
            or any(type(value) not in (int,float) or not 0 <= value <= 1 for value in target.get('fraction',[.5,.5]))):
        raise ValueError('invalid Mirror target mouse recipe')
    if kind not in ('box', 'extrusion') and target['face'] != 0:
        raise ValueError('Mirror face outside target')
    if kind == 'extrusion' and target['face'] > 3:
        raise ValueError('Mirror extrusion wall outside target')
    if kind in ('box', 'extrusion'):
        a,b,c,d = target['corners']
        if (not (a[0] < c[0] and a[1] < c[1])
                or b != [c[0],a[1],a[2]] or c[2] != a[2] or d != [a[0],c[1],a[2]]):
            raise ValueError('Mirror prism requires a horizontal rectangular base')


def target_geometry(target, host):
    Rhino = host['Rhino']
    points = [host['_point'](p) for p in target['corners']]
    kind = target['kind']
    if kind in ('box', 'extrusion'):
        # The independently prescribed horizontal rectangle is [20,-5,1]
        # through [30,5,1], with a six-unit extrusion in world Z.
        if kind == 'box':
            return Rhino.Geometry.Brep.CreateFromBox(Rhino.Geometry.BoundingBox(points[0], points[2]+Rhino.Geometry.Vector3d(0,0,6)))
        profile = Rhino.Geometry.PolylineCurve(points+[points[0]])
        try:
            result = Rhino.Geometry.Extrusion.Create(profile, 6., True)
            if result is None: raise ValueError('Mirror extrusion creation failed')
            return result
        finally: profile.Dispose()
    if kind == 'trimmed':
        curve = Rhino.Geometry.PolylineCurve(points+[points[0]])
        try:
            faces = Rhino.Geometry.Brep.CreatePlanarBreps(curve, 1e-9)
            if faces is None or len(faces) != 1: raise ValueError('Mirror trim creation failed')
            return faces[0]
        finally: curve.Dispose()
    if kind == 'mesh':
        mesh = Rhino.Geometry.Mesh()
        for point in points: mesh.Vertices.Add(point)
        mesh.Faces.AddFace(0,1,2,3)
        mesh.Normals.ComputeNormals()
        return mesh
    if kind == 'curve': return Rhino.Geometry.PolylineCurve(points+[points[0]])
    return Rhino.Geometry.NurbsSurface.CreateFromCorners(*points)


def target_snapshot(key, host):
    Rhino = host['Rhino']; doc = Rhino.RhinoDoc.ActiveDoc
    obj = doc.Objects.FindId(key)
    if obj is None: raise ValueError('Mirror plane target disappeared')
    geometry = obj.Geometry
    box = geometry.GetBoundingBox(True)
    result = dict(type=str(geometry.ObjectType), selected=bool(obj.IsSelected(False)),
                  bounds=[host['_xyz'](box.Min), host['_xyz'](box.Max)],
                  name=obj.Attributes.Name, groups=list(obj.Attributes.GetGroupList() or []),
                  layer=int(obj.Attributes.LayerIndex), color_source=str(obj.Attributes.ColorSource),
                  color=[int(obj.Attributes.ObjectColor.R),int(obj.Attributes.ObjectColor.G),int(obj.Attributes.ObjectColor.B)])
    if isinstance(geometry, Rhino.Geometry.Extrusion):
        brep = geometry.ToBrep()
        try: result['definition'] = host['_interchange_brep_record'](brep, include_samples=False)
        finally: brep.Dispose()
    elif isinstance(geometry, Rhino.Geometry.Surface):
        result['definition'] = host['_nurbs_surface_definition'](geometry)
    elif isinstance(geometry, Rhino.Geometry.Brep):
        result['definition'] = host['_interchange_brep_record'](geometry, include_samples=False)
    elif isinstance(geometry, Rhino.Geometry.Curve):
        result['definition'] = host['_nurbs_curve_definition'](geometry)
    elif isinstance(geometry, Rhino.Geometry.Mesh):
        result['vertices'] = [host['_xyz'](point) for point in geometry.Vertices]
        result['faces'] = [[int(face.A),int(face.B),int(face.C)] + ([int(face.D)] if face.IsQuad else []) for face in geometry.Faces]
    else: raise ValueError('unexpected Mirror plane target type')
    return result


def run_with_target(operation, host, action):
    validate_target(operation['mirror_target'])
    Rhino, System = host['Rhino'], host['System']; doc = Rhino.RhinoDoc.ActiveDoc
    geometry = target_geometry(operation['mirror_target'], host)
    key = System.Guid.Empty
    try:
        if geometry is None or not geometry.IsValid: raise ValueError('invalid Mirror plane target geometry')
        key = doc.Objects.Add(geometry)
        if key == System.Guid.Empty: raise ValueError('Mirror plane target insertion failed')
        target = dict(operation['mirror_target'], id=key)
        before = target_snapshot(key, host)
        value, elapsed = action(operation, host, target)
        value['target'] = dict(before=before, after=target_snapshot(key, host))
        return value, elapsed
    finally:
        if key != System.Guid.Empty and not doc.Objects.Delete(key, True):
            raise ValueError('Mirror plane target cleanup failed')
        if geometry is not None: geometry.Dispose()


def click_target(operation, target, host, script):
    """Deliver one real owned face/border click, with no completion Enter."""
    import os
    import clr
    clr.AddReference('System.Windows.Forms')
    from System.Windows.Forms import Timer
    from shrink_face_input import _input_hooks
    Rhino, System = host['Rhino'], host['System']; doc = Rhino.RhinoDoc.ActiveDoc
    Rhino.RhinoApp.RunScript('_SetView _World _'+target['view'], False)
    Rhino.RhinoApp.RunScript('_Zoom _Extents', False)
    view = doc.Views.ActiveView; viewport = view.ActiveViewport
    viewport.DisplayMode = Rhino.Display.DisplayModeDescription.FindByName(target.get('display','Shaded'))
    geometry = doc.Objects.FindId(target['id']).Geometry
    temporary = None
    if isinstance(geometry, Rhino.Geometry.Extrusion):
        temporary = geometry.ToBrep(); face = temporary.Faces[target['face']]
    elif isinstance(geometry, Rhino.Geometry.Brep): face = geometry.Faces[target['face']]
    else: face = geometry
    try:
        if isinstance(face, Rhino.Geometry.Surface):
            # Use an interior point to avoid adjacent face selection ambiguity.
            u, v = target.get('fraction',[.5,.5])
            point = face.PointAt(face.Domain(0).ParameterAt(u), face.Domain(1).ParameterAt(v))
        else:
            point = host['_point']([sum(p[axis] for p in target['corners'])/4 for axis in range(3)])
        doc.Views.Redraw()
        pixel = viewport.WorldToClient(point)
        x, y = int(pixel.X), int(pixel.Y)
        if not 1 <= x < viewport.Size.Width-1 or not 1 <= y < viewport.Size.Height-1:
            raise ValueError('Mirror target pick outside owned viewport')
        # Certify that the ray meets the intended planar face before input.
        if isinstance(face, Rhino.Geometry.BrepFace):
            ok, line = viewport.GetFrustumLine(x, y)
            if not ok: raise ValueError('Mirror face pick ray unavailable')
            ray = Rhino.Geometry.LineCurve(line)
            try:
                ok, curves, intersections = Rhino.Geometry.Intersect.Intersection.CurveBrepFace(ray, face, doc.ModelAbsoluteTolerance)
                for curve in curves or []: curve.Dispose()
                if not ok or not intersections: raise ValueError('Mirror ray missed prescribed face')
            finally: ray.Dispose()
        screen = view.ClientToScreen(System.Drawing.Point(x,y))
    finally:
        if temporary is not None: temporary.Dispose()
    timer = Timer(); timer.Interval = 100
    active, sent, clicked, cancelled, menu = [], [], [], [], []
    start = System.DateTime.UtcNow
    progress = os.path.join(os.path.dirname(os.path.abspath(host['__file__'])), 'worker-progress.log')
    def begun(sender, event):
        if event.CommandEnglishName == 'Mirror': active.append(True)
    def ended(sender, event):
        if event.CommandEnglishName == 'Mirror': active[:] = []
    def tick(sender, event):
        if not active: return
        if sent:
            if target.get('menu') == 'First' and clicked and not menu and (System.DateTime.UtcNow-sent[0]).TotalSeconds > .8:
                with open(progress, 'a') as stream:
                    stream.write('PICK @component-menu:%s:First 1 1\n' % operation['id']); stream.flush()
                menu.append(True); sent[0] = System.DateTime.UtcNow
                return
            # Rejected picks leave GetObject active and do not consume the
            # script's Pause. Deliver the recipe's explicit Cancel after the
            # owned click; never manufacture a selection or accept with Enter.
            if operation['finish'] == 'Cancel' and clicked and len(cancelled) < 2 and (System.DateTime.UtcNow-sent[0]).TotalSeconds > 2.0:
                cancel_id = operation['id'] if not cancelled else operation['id'][:80]+'-after-menu'
                with open(progress, 'a') as stream:
                    stream.write('PICK @component-finish:%s:Cancel 1 1\n' % cancel_id); stream.flush()
                cancelled.append(True)
                sent[0] = System.DateTime.UtcNow
            return
        if Rhino.Input.RhinoGet.InGetObject(doc):
            name = '@component-click:%s:0:sub' % operation['id'] if target.get('modifiers','plain') == 'sub' or target['pick'] == 'mouse-sub' else '@mirror-target:'+operation['id']
            with open(progress, 'a') as stream:
                stream.write('PICK %s %d %d\n' % (name, screen.X, screen.Y)); stream.flush()
            sent.append(System.DateTime.UtcNow)
        elif (System.DateTime.UtcNow-start).TotalSeconds > 15:
            with open(progress, 'a') as stream:
                stream.write('PICK_ABORT %s\n' % operation['id']); stream.flush()
            timer.Stop()
    class MouseEvents(Rhino.UI.MouseCallback):
        def OnMouseDown(self, event):
            if sent:
                clicked.append(True)
                host['_record_progress']('Mirror target mouse-down viewport=%s point=%d,%d ctrl=%s shift=%s' %
                    (str(event.View.ActiveViewport.Id),event.ViewportPoint.X,event.ViewportPoint.Y,event.CtrlKeyDown,event.ShiftKeyDown))
    mouse = MouseEvents()
    with _input_hooks(timer, mouse, [(timer.Tick,tick),(Rhino.Commands.Command.BeginCommand,begun),(Rhino.Commands.Command.EndCommand,ended)]):
        result = Rhino.RhinoApp.RunScript(script, True)
    if not sent or not clicked: raise ValueError('Mirror target mouse input incomplete')
    return result
