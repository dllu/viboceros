"""Move the pointer only in the owned Rhino window; acknowledge real hover."""
import json,subprocess,time
from .group_picking import IdlePicker
from .client import OracleProtocolError
from .subcurve_direction_probe import validate_request
class SubcurveDirectionPicker(IdlePicker):
    def __init__(self,request):
        validate_request(request);super().__init__();self.cases={'@subcurve-direction:'+o['id']:o for o in request['operations']};self.moved={}
    def __call__(self,job,pids):self.job=job;super().__call__(job,pids)
    def send_input(self,name,x,y,window):
        if name not in self.cases:raise OracleProtocolError('unexpected SubCrv direction marker')
        op=self.cases[name]
        if name not in self.moved:
            subprocess.run(['xdotool','windowactivate','--sync',window,'mousemove',str(int(x)+1),y,'mousemove',x,y],check=True,timeout=10);self.moved[name]=time.monotonic();return False
        ready=self.job/('subcurve-direction-ready-'+op['id']+'.json')
        if not ready.exists():
            if time.monotonic()-self.moved[name]<1.:return False
            subprocess.run(['xdotool','windowactivate','--sync',window,'mousemove',str(int(x)+1),y,'mousemove',x,y],check=True,timeout=10);self.moved[name]=time.monotonic();return False
        if json.loads(ready.read_text())!=name:raise OracleProtocolError('foreign direction acknowledgement')
        # The owned worker sends the bounded tokens with public SendKeystrokes.
        # Linux keyboard synthesis can stall Wine's modal curve getter.
        return True
    def record_diagnostics(self,response):
        if self.seen!=set(self.cases):raise OracleProtocolError('incomplete direction inputs')
