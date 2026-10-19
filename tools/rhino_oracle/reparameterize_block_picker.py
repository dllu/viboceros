"""An explicitly requested native overlap choice follows the owned leaf click."""
import subprocess
import time
import json
import os
from .catenary_point_picker import CatenaryPointPicker

class BlockReferencePicker(CatenaryPointPicker):
    def __init__(self, request):
        super().__init__(request)
        self.choices={'@catenary-click:'+op['id']+'-0':op['selection_choice']
                      for op in request['operations'] if 'selection_choice' in op}
        for op in request['operations']:
            for index,choice in enumerate(op.get('selection_choices',[])):
                self.choices['@catenary-click:'+op['id']+'-'+str(index)]=choice
        self.active=None
        self.aborted=[]
    def __call__(self,job,pids):
        self.job=job
        super().__call__(job,pids)
        if self.active is None:return
        name,window,started,attempts=self.active
        case=self.cases[name].rsplit('-',1)[0]
        progress=job/'worker-progress.log'
        if progress.exists() and case+': getter returned' in progress.read_text():
            self.active=None;return
        if time.monotonic()-started < (40. if attempts==0 else 1.):return
        if attempts>=3:raise RuntimeError('owned block input did not cancel')
        subprocess.run(['xdotool','windowactivate','--sync',window,'key','--clearmodifiers',
                        '--delay','100','Escape','Escape'],check=True,timeout=10)
        if name not in self.aborted:self.aborted.append(name)
        self.active=[name,window,time.monotonic(),attempts+1]
    def send_input(self, name, x, y, window):
        if not super().send_input(name,x,y,window):return False
        if name in self.choices:
            time.sleep(.3)
            subprocess.run(['xdotool','key','--clearmodifiers','--delay','100','Home']+
                           ['Down']*self.choices[name]+['Return'],check=True,timeout=10)
        path=self.job/('block-selection-ready-'+self.cases[name]+'.json')
        temporary=path.with_suffix('.tmp');temporary.write_text(json.dumps(name));os.replace(temporary,path)
        self.active=[name,window,time.monotonic(),0]
        return True
    def record_diagnostics(self,response):
        super().record_diagnostics(response)
        response['block_controller_aborts']=list(self.aborted)
