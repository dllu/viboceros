"""Acknowledged Escape input restricted to the owned private Rhino window."""
from contextlib import contextmanager
import re
import subprocess

@contextmanager
def guard(host, case, identifier):
    import os
    import Eto.Forms as Forms
    R,S=host['Rhino'],host['System']
    timer=Forms.UITimer();timer.Interval=.2
    started=S.DateTime.UtcNow;receipts=[];errors=[];last=[0.]
    control=case.startswith('control_bounded_') or case in ('cancel_plane','cancel_three_point','cancel_curve','default')
    progress=os.path.join(os.path.dirname(host['__file__']),'worker-progress.log')
    def tick(sender,args):
        elapsed=(S.DateTime.UtcNow-started).TotalSeconds
        prompt=str(R.RhinoApp.CommandPrompt)
        expected=prompt.lower().startswith('next point') if case.endswith('cancel_curve') else prompt.lower().startswith('select surface to draw curve on')
        timed_out=elapsed>60.
        if (control and expected or timed_out) and elapsed>=1. and elapsed-last[0]>=1. and R.Commands.Command.InCommand() and len(receipts)<4:
            if timed_out and not errors:errors.append('owned infinite-plane input did not complete')
            number=len(receipts);last[0]=elapsed
            receipts.append(dict(prompt=prompt,elapsed=elapsed,action='EscapeRemainingCommand',sequence=number))
            S.IO.File.AppendAllText(progress,'\nINTERPOLATION_PLANE_ESCAPE %s %d\n'%(identifier,number))
    timer.Elapsed+=tick;timer.Start()
    try:
        yield receipts,errors
    finally:
        timer.Stop();timer.Elapsed-=tick;timer.Dispose()

class PlaneKeyboard:
    def __init__(self, request):
        from .client import OracleProtocolError
        from .surface_interpolate_plane_cases import validate
        if type(request.get('iterations',1)) is not int or request.get('iterations',1)!=1 or not 1<=len(request.get('operations',[]))<=64:
            raise OracleProtocolError('bounded one-iteration plane interpolation input required')
        self.ids=[op['id'] for op in request['operations']]
        if len(set(self.ids))!=len(self.ids):raise OracleProtocolError('unique plane interpolation input ids required')
        for op in request['operations']:validate(op)
        self.sent=set()

    def __call__(self, job, owned_pids):
        from .client import OracleProtocolError,_read_optional_text,_rhino_window_for_pids
        progress=_read_optional_text(job/'worker-progress.log')
        for name,number in re.findall(r'^INTERPOLATION_PLANE_ESCAPE ([A-Za-z0-9_-]{1,80}) ([0-3])$',progress,re.MULTILINE):
            key=(name,int(number))
            if name not in self.ids:raise OracleProtocolError('unexpected interpolation plane Escape marker')
            if key in self.sent or not owned_pids:continue
            window=_rhino_window_for_pids(owned_pids)
            if window is None:continue
            subprocess.run(['xdotool','windowactivate','--sync',window,'key','--clearmodifiers','Escape'],check=True,timeout=10)
            self.sent.add(key)

    def record_diagnostics(self,response):
        from .client import OracleProtocolError
        if [r['id'] for r in response['results']]!=self.ids:raise OracleProtocolError('incomplete plane input coverage')
        expected=set()
        for row in response['results']:
            if not row['value'].get('returned_idle'):raise OracleProtocolError('plane interpolation did not return to idle')
            receipts=row['value']['cancel_guard']
            for event in receipts:expected.add((row['id'],event['sequence']))
            row['value']['host_input']=dict(escapes=len(receipts),private_owned_window=True)
        if self.sent!=expected:raise OracleProtocolError('incomplete acknowledged interpolation plane Escape input')
