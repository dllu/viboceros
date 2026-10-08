"""Bounded cycle clicks inside the newly owned Rhino window."""
import subprocess,time
from .group_picking import IdlePicker
from .boolean_two_open_probe import validate_request,SPECS
from .client import OracleProtocolError
class BooleanTwoOpenPicker(IdlePicker):
    def __init__(self,request):
        validate_request(request);super().__init__()
        self.cases={'@boolean-two:'+o['id']:SPECS[o['case']] for o in request['operations']}
        self.state={}
    def __call__(self,job,pids):
        self.job=job;super().__call__(job,pids)
    def send_input(self,name,x,y,window):
        if name not in self.cases:raise OracleProtocolError('unexpected Boolean2Objects marker')
        if ('PICK_DONE '+name+'\n') in (self.job/'worker-progress.log').read_text():return True
        case=self.cases[name];now=time.monotonic()
        state=self.state.setdefault(name,dict(window=window,x=x,y=y,count=0,ready=now+1.5))
        if (state['window'],state['x'],state['y'])!=(window,x,y):raise OracleProtocolError('Boolean2Objects owned target changed')
        if now<state['ready']:return False
        argv=['xdotool','windowactivate','--sync',window]
        if state['count']<case['cycles']:
            subprocess.run(argv+['mousemove',x,y,'click','1'],check=True,timeout=10)
            state['count']+=1;state['ready']=now+.8;return False
        subprocess.run(argv+['key','--clearmodifiers','Escape' if case.get('cancel') else 'Return'],check=True,timeout=10)
        return True
