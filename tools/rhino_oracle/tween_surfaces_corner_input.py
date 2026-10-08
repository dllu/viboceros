"""Prime native input timers with owned pointer motion before corner clicks."""
import re,subprocess,time
from .group_picking import IdlePicker
from .client import _read_optional_text,_rhino_window_for_pids,OracleProtocolError
from .tween_surfaces_corners_probe import validate_request
class CornerPicker(IdlePicker):
    def __init__(self,request):
        validate_request(request);super().__init__();self.expected={'@tween-corner:'+o['id']for o in request['operations'] if o['case']!='baseline'};self.moved={}
    def __call__(self,job,pids):
        progress=_read_optional_text(job/'worker-progress.log')
        for name,x,y in re.findall(r'^MOVE (\S+) (\d+) (\d+)$',progress,re.MULTILINE):
            if name not in self.expected:raise OracleProtocolError('foreign corner motion marker')
            if name in self.seen:continue
            ready=self.ready.setdefault(name,time.monotonic()+.5)
            if time.monotonic()<ready:continue
            window=_rhino_window_for_pids(pids)
            if window is None:continue
            self.send_input(name,x,y,window)
            temporary=job/'click-ack.json.tmp';temporary.write_text(__import__('json').dumps(name),encoding='utf-8');__import__('os').replace(temporary,job/'click-ack.json')
            self.seen.add(name)
        super().__call__(job,pids)
    def record_diagnostics(self,response):
        if self.seen!=self.expected:raise OracleProtocolError('incomplete native corner clicks')
