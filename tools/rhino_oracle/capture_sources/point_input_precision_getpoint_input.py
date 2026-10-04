"""Owned-window coordinate typing after acknowledged public GetPoint motion."""
import json
import subprocess
import time

from .client import OracleProtocolError
from .group_picking import IdlePicker
from .point_input_precision_probe import recipe,validate_request


class PointInputPrecisionPicker(IdlePicker):
    def __init__(self,request):
        validate_request(request)
        super().__init__()
        self.cases = {'@point-input-precision:'+op['id']:op for op in request['operations']}
        self.moved = {}

    def __call__(self,job,owned_pids):
        self.job = job
        super().__call__(job,owned_pids)

    def send_input(self,name,x,y,window):
        if name not in self.cases: raise OracleProtocolError('unexpected precision input marker')
        op = self.cases[name]
        if name not in self.moved:
            subprocess.run(['xdotool','windowactivate','--sync',window,'mousemove',str(int(x)+1),y,
                            'mousemove',x,y],check=True,timeout=10)
            self.moved[name] = (window,x,y,time.monotonic())
            return False
        old_window,old_x,old_y,moved = self.moved[name]
        if (window,x,y)!=(old_window,old_x,old_y): raise OracleProtocolError('owned precision input changed')
        ready = self.job/('point-input-precision-ready-'+op['id']+'.json')
        if not ready.exists():
            if time.monotonic()-moved > 1.:
                subprocess.run(['xdotool','windowactivate','--sync',window,'mousemove',str(int(x)+1),y,
                                'mousemove',x,y],check=True,timeout=10)
                self.moved[name] = (window,x,y,time.monotonic())
            return False
        if json.loads(ready.read_text())!=name: raise OracleProtocolError('foreign precision motion acknowledgement')
        if time.monotonic()-moved < .2: return False
        subprocess.run(['xdotool','windowactivate','--sync',window,'type','--clearmodifiers','--delay','10',
                        recipe(op)['token']],check=True,timeout=10)
        time.sleep(.25)
        subprocess.run(['xdotool','windowactivate','--sync',window,'key','--clearmodifiers','Return'],check=True,timeout=10)
        return True

    def record_diagnostics(self,response):
        if set(self.cases)!=self.seen: raise OracleProtocolError('incomplete precision inputs')
        rows = response.get('results')
        if (not isinstance(rows,list) or any(not isinstance(row,dict) for row in rows)
                or [r.get('id') for r in rows]!=[op['id'] for op in self.cases.values()]):
            raise OracleProtocolError('precision response differs from prescribed inputs')
