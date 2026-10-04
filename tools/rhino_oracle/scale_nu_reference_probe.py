"""Owned ScaleNU reference getters with prescribed cursor and typed target input."""
import json
import os
import re


def validate_request(request):
    if (not isinstance(request, dict) or type(request.get('protocol_version')) is not int
            or request['protocol_version'] != 1 or type(request.get('iterations', 1)) is not int
            or request.get('iterations', 1) != 1 or not isinstance(request.get('operations'), list)
            or not 1 <= len(request['operations']) <= 32):
        raise ValueError('ScaleNU reference capture requires bounded protocol 1 recipes')
    names = set()
    for op in request['operations']:
        if (not isinstance(op, dict) or set(op) != {'op', 'id', 'axis', 'view', 'aim', 'finish', 'reference'}
                or op['op'] != 'scale_nu_reference' or not isinstance(op['id'], str)
                or re.match(r'^[A-Za-z0-9_-]{1,80}\Z', op['id']) is None or op['id'] in names
                or type(op['axis']) is not int or not 0 <= op['axis'] < 3
                or op['view'] not in ('Top', 'Front', 'Perspective')
                or op['aim'] not in ('Near', 'Far', 'Negative', 'OffAxis')
                or op['finish'] not in ('Click', 'Typed', 'Scripted')
                or op['reference'] not in ('Axis', 'OffAxis')):
            raise ValueError('invalid ScaleNU reference recipe')
        names.add(op['id'])


def request():
    rows = []
    for axis, view in ((0, 'Top'), (1, 'Top'), (2, 'Front'), (0, 'Perspective')):
        for aim in ('Near', 'Far'):
            for finish in ('Click', 'Typed'):
                rows.append((axis, view, aim, finish, 'Axis'))
    rows += [(0, 'Top', 'OffAxis', 'Click', 'Axis'),
             (0, 'Top', 'Negative', 'Click', 'Axis'),
             (0, 'Top', 'Far', 'Click', 'OffAxis'),
             (0, 'Top', 'Far', 'Typed', 'OffAxis')]
    rows += [(axis, view, aim, 'Scripted', 'Axis')
             for axis, view in ((0, 'Top'), (1, 'Top'), (2, 'Front'), (0, 'Perspective'))
             for aim in ('Near', 'Far')]
    rows += [(0, 'Top', 'OffAxis', 'Scripted', 'Axis'),
             (0, 'Top', 'Negative', 'Scripted', 'Axis'),
             (0, 'Top', 'Near', 'Scripted', 'OffAxis'),
             (0, 'Top', 'Far', 'Scripted', 'OffAxis')]
    return dict(protocol_version=1, iterations=1, operations=[
        dict(op='scale_nu_reference', id='scale-nu-reference-'+str(i), axis=axis,
             view=view, aim=aim, finish=finish, reference=reference)
        for i, (axis, view, aim, finish, reference) in enumerate(rows)])


def recipe(op):
    axis = op['axis']
    reference = [0., 0., 0.]
    reference[axis] = 2.
    if op['reference'] == 'OffAxis':
        reference = [1., 1., 1.]
    aim = [0., 0., 0.]
    aim[axis] = {'Near': 4., 'Far': 8., 'Negative': -4., 'OffAxis': 4.}[op['aim']]
    if op['aim'] == 'OffAxis':
        aim[(axis+1) % 3] = 5.
    target = [0., 0., 0.]
    target[axis] = 6.
    prefix = '_ScaleNU _Copy=_No w0,0,0 ' + '1 '*axis + 'w'+','.join(str(x) for x in reference)+' '
    target = 'w'+','.join(str(x) for x in target)
    return dict(reference=reference, aim=aim, typed_target=target,
                macro=prefix+(target if op['finish'] == 'Scripted' else '_Pause')+' 1'*(2-axis))


def run(op, host):
    validate_request(dict(protocol_version=1, operations=[op]))
    Rhino, System = host['Rhino'], host['System']
    doc = Rhino.RhinoDoc.ActiveDoc
    if Rhino.Commands.Command.InCommand():
        raise ValueError('ScaleNU reference history requires idle execution')
    if list(doc.Objects):
        raise ValueError('ScaleNU reference capture requires an empty owned document')
    import clr
    clr.AddReference('System.Windows.Forms')
    from System.Windows.Forms import Timer
    from shrink_face_input import _input_hooks
    from snap_environment import environment
    from viewport_capture import capture
    from named_view_policy_probe import snapshot as camera_snapshot
    from join_probe import observe_command
    view = doc.Views.ActiveView
    vp = view.ActiveViewport
    original = Rhino.DocObjects.ViewportInfo(vp)
    original_plane = vp.GetConstructionPlane()
    root = os.path.dirname(os.path.abspath(host['__file__']))
    marker = '@scale-nu-reference:'+op['id']
    ready = os.path.join(root, 'scale-nu-reference-ready-'+op['id']+'.json')
    pending, errors, active = [], [], []
    timer = Timer()
    timer.Interval = 100
    hooks_owned = False
    started = System.DateTime.UtcNow
    source = None
    def snapshot():
        return [dict(point=host['_xyz'](obj.Geometry.Location), selected=bool(obj.IsSelected(False)))
                for obj in sorted(list(doc.Objects), key=lambda o: o.RuntimeSerialNumber)]
    def begun(sender, event):
        if event.CommandEnglishName == 'ScaleNU':
            active.append(True)
    def ended(sender, event):
        if event.CommandEnglishName == 'ScaleNU':
            active[:] = []
            host['_record_progress']('REFERENCE_END '+op['id'])
    def tick(sender, event):
        if not errors and (System.DateTime.UtcNow-started).TotalSeconds > 30:
            errors.append('ScaleNU reference cursor timed out')
            host['_record_progress']('PICK_ABORT '+op['id'])
            timer.Stop()
    class Mouse(Rhino.UI.MouseCallback):
        def OnEndMouseMove(self, event):
            if (not active or pending or errors or event.View.ActiveViewport.Id != vp.Id
                    or [int(event.ViewportPoint.X), int(event.ViewportPoint.Y)] != [x, y]
                    or not Rhino.Input.RhinoGet.InGetPoint(doc)):
                return
            try:
                ok, ray = vp.GetFrustumLine(x, y)
                if not ok:
                    raise ValueError('ScaleNU reference viewing line unavailable')
                frame = capture(vp, host['_point'](spec['aim']), [x, y], host)
                frame['ray'] = [host['_xyz'](ray.From), host['_xyz'](ray.To)]
                pending.append(dict(frame=frame, camera=camera_snapshot(vp, Rhino),
                                    objects=snapshot(), prompt=Rhino.RhinoApp.CommandPrompt,
                                    phase='cursor_calibration' if op['finish'] == 'Scripted' else 'second_reference'))
                host['_record_progress']('REFERENCE_READY '+op['id'])
                with open(ready+'.tmp', 'w') as stream:
                    json.dump(marker, stream)
                os.rename(ready+'.tmp', ready)
            except Exception as error:
                errors.append(str(error))
                host['_record_progress']('PICK_ABORT '+op['id'])
    try:
        if not vp.SetProjection(getattr(Rhino.Display.DefinedViewportProjection, op['view']), 'Owned ScaleNU references', False):
            raise ValueError('ScaleNU reference view setup failed')
        vp.SetConstructionPlane(Rhino.Geometry.Plane.WorldXY)
        if not vp.ZoomBoundingBox(Rhino.Geometry.BoundingBox(host['_point']([-12.,-10.,-10.]),host['_point']([12.,10.,10.]))):
            raise ValueError('ScaleNU reference zoom failed')
        source = doc.Objects.AddPoint(host['_point']([2., 3., 4.]))
        if source == System.Guid.Empty:
            raise ValueError('ScaleNU reference source insertion failed')
        doc.Objects.Select(source)
        # Seed through a completed public invocation, preserving this source.
        if not Rhino.RhinoApp.RunScript('_ScaleNU _Copy=_No w0,0,0 1 1 1', False):
            raise ValueError('ScaleNU reference seeding failed')
        before = snapshot()
        spec = recipe(op)
        pixel = vp.WorldToClient(host['_point'](spec['aim']))
        x, y = int(pixel.X), int(pixel.Y)
        if not 1 <= x < vp.Size.Width-1 or not 1 <= y < vp.Size.Height-1:
            raise ValueError('ScaleNU reference cursor outside viewport')
        screen = view.ClientToScreen(System.Drawing.Point(x, y))
        history_before = Rhino.RhinoApp.CommandHistoryWindowText
        calibration = None
        with environment(dict(persistent_snaps=[]), host):
            hooks_owned = True
            with _input_hooks(timer, Mouse(), [(Rhino.Commands.Command.BeginCommand, begun),
                    (Rhino.Commands.Command.EndCommand, ended), (timer.Tick, tick)]):
                doc.Views.Redraw()
                host['_record_progress']('PICK '+marker+' %d %d' % (screen.X, screen.Y))
                if op['finish'] == 'Scripted':
                    getter = Rhino.Input.Custom.GetPoint()
                    try:
                        getter.SetCommandPrompt('Owned ScaleNU reference cursor calibration')
                        direction = [0., 0., 0.]
                        direction[op['axis']] = 1.
                        if not getter.Constrain(host['_point']([0., 0., 0.]), host['_point'](direction)):
                            raise ValueError('ScaleNU reference cursor constraint failed')
                        active.append(True)
                        result = getter.Get()
                        active[:] = []
                        if result != Rhino.Input.GetResult.Point:
                            raise ValueError('ScaleNU reference cursor calibration failed: '+str(result))
                        calibration = dict(point=host['_xyz'](getter.Point()), result=str(result))
                    finally:
                        getter.Dispose()
                success, after, events = observe_command(Rhino.Commands.Command, 'ScaleNU',
                    lambda: Rhino.RhinoApp.RunScript(spec['macro'], True), snapshot, lambda: [], True)
        if errors or len(pending) != 1:
            raise ValueError('ScaleNU reference observation incomplete: '+str(errors))
        after_script = snapshot()
        host['_record_progress']('REFERENCE_RESULT '+op['id']+' '+json.dumps(after_script))
        history = Rhino.RhinoApp.CommandHistoryWindowText[len(history_before):]
        Rhino.RhinoApp.RunScript('_Undo', False)
        undo = snapshot()
        Rhino.RhinoApp.RunScript('_Redo', False)
        redo = snapshot()
        return dict(before=before, pending=pending[0], after=after, after_script=after_script,
                    undo=undo, redo=redo, events=events, success=success, history=history,
                    recipe=spec, calibration=calibration), 0
    finally:
        if not hooks_owned:
            timer.Dispose()
        for obj in list(doc.Objects):
            doc.Objects.Delete(obj.Id, True)
        vp.SetViewProjection(original, False)
        vp.SetConstructionPlane(original_plane)
        original.Dispose()
        doc.Views.Redraw()
