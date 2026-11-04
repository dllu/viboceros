"""Actual idle Ctrl+Shift double-click controls, including bounded ranges."""
import re
import subprocess
import time
from contextlib import ExitStack
from .component_picking import ComponentPicker

from .edge_loop_gesture_cases import cases,filtered_cases,validate

class EdgeLoopGesturePicker(ComponentPicker):
    def __init__(self,filtered=False,modifiers_by_id=None):
        super().__init__();self.filtered=filtered;self.modifiers_by_id=modifiers_by_id or {}
    def __call__(self,job,owned_pids):
        previous=len(self.seen)
        super().__call__(job,owned_pids)
        if len(self.seen)!=previous:self.last_activity=time.monotonic()
        if hasattr(self,'last_activity') and time.monotonic()-self.last_activity>25 and not (job/'response.json').exists():
            raise ValueError('owned edge-loop gesture did not settle')
    def send_input(self,name,x,y,window):
        if re.fullmatch(r'@edge-loop-double:[A-Za-z0-9_.-]{1,100}',name) is None:
            raise ValueError('invalid owned edge-loop gesture')
        with ExitStack() as cleanup:
            subprocess.run(['xdotool','windowactivate','--sync',window],check=True,timeout=10)
            mode=self.modifiers_by_id.get(name.split(':',1)[1])
            keys=(mode,) if mode is not None else (() if self.filtered else ('ctrl','shift'))
            for key in keys:
                cleanup.callback(subprocess.run,['xdotool','keyup',key],check=True,timeout=10)
                subprocess.run(['xdotool','keydown',key],check=True,timeout=10)
            cleanup.callback(subprocess.run,['xdotool','mouseup','1'],check=True,timeout=10)
            subprocess.run(['xdotool','mousemove',str(x),str(y)],check=True,timeout=10)
            time.sleep(.15)
            subprocess.run(['xdotool','click','--repeat','2','--delay','100','1'],check=True,timeout=10)
        return True
