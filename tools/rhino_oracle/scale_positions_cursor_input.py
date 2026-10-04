"""Mouse acknowledgement and point input in the owned private Rhino window."""
import json
import subprocess
import time

from .client import OracleProtocolError
from .group_picking import IdlePicker
from .scale_positions_cursor_probe import recipe,validate_request


class ScalePositionsCursorPicker(IdlePicker):
    def __init__(self,request):
        validate_request(request)
        super().__init__()
        self.cases = {'@scale-positions-cursor:'+op['id']:op for op in request['operations']}
        self.moved = {}
        self.clicked = set()

    def __call__(self,job,owned_pids):
        self.job = job
        super().__call__(job,owned_pids)

    def send_input(self,name,x,y,window):
        if name not in self.cases: raise OracleProtocolError('unexpected ScalePositions cursor marker')
        op = self.cases[name]
        if name not in self.moved:
            subprocess.run(['xdotool','windowactivate','--sync',window,'mousemove',str(int(x)+1),y,
                            'mousemove',x,y],check=True,timeout=10)
            self.moved[name] = (window,x,y,time.monotonic())
            return False
        old_window,old_x,old_y,moved = self.moved[name]
        if (window,x,y)!=(old_window,old_x,old_y): raise OracleProtocolError('owned ScalePositions cursor changed')
        ready = self.job/('scale-positions-cursor-ready-'+op['id']+'.json')
        if not ready.exists():
            if time.monotonic()-moved > 1.:
                subprocess.run(['xdotool','windowactivate','--sync',window,'mousemove',str(int(x)+1),y,
                                'mousemove',x,y],check=True,timeout=10)
                self.moved[name] = (window,x,y,time.monotonic())
            return False
        if json.loads(ready.read_text())!=name: raise OracleProtocolError('foreign ScalePositions motion acknowledgement')
        if time.monotonic()-moved < .2: return False
        if op['finish']=='ClickCancel':
            if name not in self.clicked:
                subprocess.run(['xdotool','windowactivate','--sync',window,'click','1'],check=True,timeout=10)
                self.clicked.add(name)
                return False
            rejected = self.job/('scale-positions-cursor-rejected-'+op['id']+'.json')
            if not rejected.exists(): return False
            if json.loads(rejected.read_text())!=name: raise OracleProtocolError('foreign ScalePositions rejected-click acknowledgement')
            subprocess.run(['xdotool','windowactivate','--sync',window,'key','--clearmodifiers','Escape'],check=True,timeout=10)
        elif op['finish']=='Click':
            subprocess.run(['xdotool','windowactivate','--sync',window,'click','1'],check=True,timeout=10)
        else:
            subprocess.run(['xdotool','windowactivate','--sync',window,'type','--clearmodifiers','--delay','30',
                            recipe(op)['typed_target']],check=True,timeout=10)
            time.sleep(.25)
            subprocess.run(['xdotool','windowactivate','--sync',window,'key','--clearmodifiers','Return'],check=True,timeout=10)
        return True

    def record_diagnostics(self,response):
        if set(self.cases)!=self.seen: raise OracleProtocolError('incomplete owned ScalePositions cursor inputs')
        rows = response.get('results')
        if (not isinstance(rows,list) or any(not isinstance(row,dict) for row in rows)
                or [r.get('id') for r in rows]!=[op['id'] for op in self.cases.values()]):
            raise OracleProtocolError('ScalePositions cursor response does not match prescribed inputs')
