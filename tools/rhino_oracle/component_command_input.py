"""Bounded real mouse inputs for immediate component commands."""
import json
import os


def drive(operation, points, host, command, snapshot, states, undo_states):
    Rhino,System=host['Rhino'],host['System']
    import clr
    clr.AddReference('System.Windows.Forms')
    from System.Windows.Forms import Timer
    timer=Timer();timer.Interval=100
    root=os.path.dirname(os.path.abspath(host['__file__']))
    progress=os.path.join(root,'worker-progress.log');acknowledgement=os.path.join(root,'click-ack.json')
    start=Rhino.RhinoApp.CommandHistoryWindowText;clock=[System.DateTime.UtcNow]
    pending=[];acknowledged=[];errors=[];finished=[];undone=[];pending_kind=[]
    active=[]
    def begun(sender,event):
        if event.CommandEnglishName==command:active.append(True)
    def ended(sender,event):
        if event.CommandEnglishName==command:active[:]=[]
    def tick(sender,event):
        try:
            history=Rhino.RhinoApp.CommandHistoryWindowText
            if not history.startswith(start):raise ValueError('component history changed unexpectedly')
            elapsed=(System.DateTime.UtcNow-clock[0]).TotalSeconds
            if elapsed>15:raise ValueError('component input timed out')
            if finished or not active:return
            if pending:
                if not acknowledged:
                    try:
                        with open(acknowledgement) as stream:ack=json.load(stream)
                    except (IOError,ValueError):return
                    if ack!=pending[0]:return
                    acknowledged.append(True);clock[0]=System.DateTime.UtcNow;return
                if elapsed<.6:return
                if pending_kind[0]=='click':states.append(snapshot())
                else:undo_states.append(snapshot());undone.append(len(states))
                pending[:]=[];acknowledged[:]=[];pending_kind[:]=[]
            index=len(states)
            name='@hole-finish:%s:%s'%(operation['id'],operation['finish']);x=y=1
            kind='finish'
            if index in operation.get('undo_after',[]) and index not in undone:
                name='@component-key:%s:%d:Undo'%(operation['id'],index);kind='undo'
            elif index<len(points):
                view=Rhino.RhinoDoc.ActiveDoc.Views.ActiveView;viewport=view.ActiveViewport
                pixel=viewport.WorldToClient(points[index]);x,y=int(pixel.X),int(pixel.Y)
                if not 1<=x<viewport.Size.Width-1 or not 1<=y<viewport.Size.Height-1:raise ValueError('component pick outside owned viewport')
                screen=view.ClientToScreen(System.Drawing.Point(x,y));x,y=screen.X,screen.Y
                name='@hole:%s:%d'%(operation['id'],index)
                kind='click'
            else:finished.append(True)
            with open(progress,'a') as stream:
                stream.write('PICK %s %d %d\n'%(name,x,y));stream.flush()
            pending.append(name);pending_kind.append(kind);clock[0]=System.DateTime.UtcNow
        except Exception as error:
            errors.append(str(error));timer.Stop()
            with open(progress,'a') as stream:stream.write('PICK_ABORT %s\n'%operation['id']);stream.flush()
    timer.Tick+=tick
    Rhino.Commands.Command.BeginCommand+=begun;Rhino.Commands.Command.EndCommand+=ended
    try:
        timer.Start();result=Rhino.RhinoApp.RunScript('_'+command,True)
        if errors or len(states)!=len(points) or undone!=operation.get('undo_after',[]) or not finished:
            raise ValueError('incomplete component inputs: %s; states: %d; history: %s'%(errors,len(states),Rhino.RhinoApp.CommandHistoryWindowText[len(start):][-1500:]))
        return result
    finally:
        timer.Stop();timer.Tick-=tick;timer.Dispose()
        Rhino.Commands.Command.BeginCommand-=begun;Rhino.Commands.Command.EndCommand-=ended
