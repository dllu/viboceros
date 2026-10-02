"""Real GetMultiple inputs; selection snapshots are output evidence only."""
import json
import os


def drive(operation, points, host, selected, trace):
    Rhino, System = host['Rhino'], host['System']
    import clr
    clr.AddReference('System.Windows.Forms')
    from System.Windows.Forms import Timer
    timer = Timer(); timer.Interval = 100
    root = os.path.dirname(os.path.abspath(host['__file__']))
    progress = os.path.join(root, 'worker-progress.log')
    acknowledgement = os.path.join(root, 'click-ack.json')
    start = Rhino.RhinoApp.CommandHistoryWindowText
    clock = [System.DateTime.UtcNow]
    pending, acknowledged, errors, finished = [], [], [], []
    def emit(name, x=1, y=1):
        with open(progress, 'a') as stream:
            stream.write('PICK %s %d %d\n' % (name, x, y)); stream.flush()
        pending.append(name); clock[0] = System.DateTime.UtcNow
    def tick(sender, event):
        try:
            history = Rhino.RhinoApp.CommandHistoryWindowText
            if not history.startswith(start): raise ValueError('component history changed unexpectedly')
            elapsed = (System.DateTime.UtcNow - clock[0]).TotalSeconds
            if elapsed > 15: raise ValueError('component input timed out')
            if finished: return
            if 'Select edges to unjoin' not in history[len(start):]: return
            if pending:
                if not acknowledged:
                    try:
                        with open(acknowledgement) as stream: ack = json.load(stream)
                    except (IOError, ValueError): return
                    if ack != pending[0]: return
                    acknowledged.append(True); clock[0] = System.DateTime.UtcNow
                    return
                if elapsed < .6: return
                trace.append(selected())
                pending[:] = []; acknowledged[:] = []
            index = len(trace)
            if index == len(operation['steps']):
                if not finished:
                    emit('@hole-finish:%s:%s' % (operation['id'], operation['finish']))
                    finished.append(True)
                return
            step = operation['steps'][index]
            if step['kind'] == 'key':
                emit('@component-key:%s:%d:%s' % (operation['id'], index, step['value']))
                return
            view = Rhino.RhinoDoc.ActiveDoc.Views.ActiveView
            viewport = view.ActiveViewport
            screens = []
            for point in points[index]:
                pixel = viewport.WorldToClient(point)
                x, y = int(pixel.X), int(pixel.Y)
                if not 1 <= x < viewport.Size.Width - 1 or not 1 <= y < viewport.Size.Height - 1:
                    raise ValueError('component input outside owned viewport')
                screens.append(view.ClientToScreen(System.Drawing.Point(x, y)))
            name = '@component-%s:%s:%d:%s' % (step['kind'], operation['id'], index, step['modifiers'])
            if step['kind'] == 'window': name += ':%d:%d' % (screens[1].X, screens[1].Y)
            emit(name, screens[0].X, screens[0].Y)
        except Exception as error:
            errors.append(str(error)); timer.Stop()
            with open(progress, 'a') as stream:
                stream.write('PICK_ABORT %s\n' % operation['id']); stream.flush()
    timer.Tick += tick
    try:
        timer.Start()
        result = Rhino.RhinoApp.RunScript('_UnjoinEdge _Pause', True)
        # Native None leaves GetMultiple and invokes SelNone. Observe that
        # final delivered input after RunScript returns; no finish key is sent.
        if (not errors and operation['steps'][-1] == dict(kind='key', value='None')
                and len(trace)+1 == len(operation['steps'])
                and 'Command: _SelNone' in Rhino.RhinoApp.CommandHistoryWindowText[len(start):]
                and pending == ['@component-key:%s:%d:None' % (operation['id'], len(trace))]):
            trace.append(selected()); finished.append(True)
        if errors or len(trace) != len(operation['steps']) or not finished:
            raise ValueError('incomplete component sequence: %s; trace: %s; history: %s' % (errors, trace, Rhino.RhinoApp.CommandHistoryWindowText[len(start):][-2000:]))
        return result
    finally:
        timer.Stop(); timer.Tick -= tick; timer.Dispose()
