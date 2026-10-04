"""Escape only an acknowledged rejected mesh getter in the owned Rhino window."""
import re
import subprocess
import time

from .client import OracleProtocolError
from .group_picking import IdlePicker
from .scale_by_plane_object_probe import validate_request


class ScaleByPlaneObjectPicker(IdlePicker):
    def __init__(self,request):
        validate_request(request); super().__init__()
        self.ids = [op['id'] for op in request['operations']]
        self.meshes = {op['id'] for op in request['operations'] if op['target']=='mesh'}

    def __call__(self,job,owned_pids):
        if self.ready and time.monotonic()-min(self.ready.values())>15:
            # Previous completed recipes have no pending input.
            pending = set(self.ready)-self.seen
            if pending and any(time.monotonic()-self.ready[name]>15 for name in pending):
                raise OracleProtocolError('ScaleByPlane mesh cancellation timed out')
        super().__call__(job,owned_pids)

    def send_input(self,name,x,y,window):
        match = re.fullmatch(r'@scale-object-cancel:([A-Za-z0-9_-]{1,80}):(0)',name)
        if not match or match[1] not in self.meshes:
            raise OracleProtocolError('foreign ScaleByPlane Object cancellation marker')
        subprocess.run(['xdotool','windowactivate','--sync',window,'key','--clearmodifiers','Escape'],
                       check=True,timeout=10)
        return True

    def record_diagnostics(self,response):
        if self.aborted or [row['id'] for row in response['results']]!=self.ids:
            raise OracleProtocolError('incomplete ScaleByPlane Object response')
        expected = set()
        for row in response['results']:
            inputs = row['value']['cancel_inputs']
            if row['id'] in self.meshes:
                if len(inputs)!=1:
                    raise OracleProtocolError('missing native mesh cancellation acknowledgement')
                markers = ['@scale-object-cancel:'+row['id']+':'+str(i) for i in range(len(inputs))]
                if [event['marker'] for event in inputs]!=markers:
                    raise OracleProtocolError('mesh cancellation acknowledgement differs from input')
                expected.update(markers)
            elif inputs: raise OracleProtocolError('unexpected mesh cancellation input')
        if self.seen!=expected: raise OracleProtocolError('incomplete mesh cancellation input')
