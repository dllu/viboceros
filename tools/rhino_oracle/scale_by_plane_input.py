"""Owned, acknowledged viewport selection for ScaleByPlane FromView."""
import json
import subprocess
import time

from .client import OracleProtocolError
from .group_picking import IdlePicker
from .scale_by_plane_probe import validate_request


class ScaleByPlaneViewPicker(IdlePicker):
    def __init__(self,request):
        validate_request(request)
        super().__init__()
        self.ids = [op['id'] for op in request['operations']]
        self.cases = {'@scale-by-plane-view:'+op['id']:op['id'] for op in request['operations'] if op['plane']=='FromView'}
        self.moved = {}

    def __call__(self,job,owned_pids):
        self.job = job
        super().__call__(job,owned_pids)

    def send_input(self,name,x,y,window):
        if name not in self.cases: raise OracleProtocolError('unexpected ScaleByPlane viewport marker')
        if name not in self.moved or time.monotonic()-self.moved[name][3]>1.:
            if name in self.moved and self.moved[name][:3]!=(window,x,y):
                raise OracleProtocolError('ScaleByPlane owned viewport changed')
            subprocess.run(['xdotool','windowactivate','--sync',window,'mousemove',str(int(x)+1),y,'mousemove',x,y],check=True,timeout=10)
            self.moved[name] = (window,x,y,time.monotonic())
            return False
        if self.moved[name][:3]!=(window,x,y): raise OracleProtocolError('ScaleByPlane owned viewport changed')
        ready = self.job/('scale-by-plane-view-ready-'+self.cases[name]+'.json')
        if not ready.exists(): return False
        if json.loads(ready.read_text())!=name: raise OracleProtocolError('foreign ScaleByPlane viewport acknowledgement')
        if time.monotonic()-self.moved[name][3]<.2: return False
        subprocess.run(['xdotool','windowactivate','--sync',window,'click','1'],check=True,timeout=10)
        return True

    def record_diagnostics(self, response):
        if self.aborted or self.seen != set(self.cases):
            raise OracleProtocolError('incomplete ScaleByPlane viewport selection')
        if [row['id'] for row in response['results']] != self.ids:
            raise OracleProtocolError('ScaleByPlane response order changed')
        for row in response['results']:
            value = row['value']
            events = [e for e in value['events'] if e['name'] == 'ScaleByPlane']
            if len(events) != 1 or events[0]['result'] != 'Success':
                raise OracleProtocolError('ScaleByPlane command did not succeed')
            if row['id'] in self.cases.values():
                if (len(value['view_pick']) != 1 or len(value['view_clicked']) != 1
                        or value['view_pick'][0]['view'] != value['view_clicked'][0]['view']):
                    raise OracleProtocolError('ScaleByPlane viewport acknowledgement differs from click')
