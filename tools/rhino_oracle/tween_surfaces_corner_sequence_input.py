"""Deliver only the bounded calibrated points of an owned corner sequence."""
import json,os,re,time,subprocess
from .group_picking import IdlePicker
from .client import _read_optional_text,_rhino_window_for_pids,OracleProtocolError
from .tween_surfaces_corner_sequences_probe import validate_request,SPECS
class SequencePicker(IdlePicker):
    def __init__(self,request):
        validate_request(request);super().__init__()
        self.completed=set()
        self.expected={'@tween-corner:'+o['id']+'-'+str(i)for o in request['operations']for i in range(len(SPECS[o['case']]['clicks']))}
    def __call__(self,job,pids):
        progress=_read_optional_text(job/'worker-progress.log')
        for name,x,y in re.findall(r'^MOVE (\S+) (\d+) (\d+)$',progress,re.MULTILINE):
            if name not in self.expected:raise OracleProtocolError('foreign corner sequence marker')
            if name in self.seen:continue
            prefix,index=name.rsplit('-',1);index=int(index)
            op_id=prefix.split(':',1)[1]
            if index>0 and not (job/('corner-sequence-clicked-'+op_id+'-'+str(index-1)+'.json')).exists():continue
            ready=self.ready.setdefault(name,time.monotonic()+.5)
            if time.monotonic()<ready:continue
            window=_rhino_window_for_pids(pids)
            if window is None:continue
            self.send_input(name,x,y,window)
            tmp=job/'click-ack.json.tmp';tmp.write_text(json.dumps(name),encoding='utf-8');os.replace(tmp,job/'click-ack.json');self.seen.add(name)
        for op_id in {name.split(':',1)[1].rsplit('-',1)[0]for name in self.expected}:
            names=sorted([name for name in self.expected if name.startswith('@tween-corner:'+op_id+'-')],key=lambda s:int(s.rsplit('-',1)[1]))
            last=job/('corner-sequence-clicked-'+op_id+'-'+str(len(names)-1)+'.json')
            if op_id in self.completed or not all(n in self.seen for n in names) or not last.exists():continue
            window=_rhino_window_for_pids(pids)
            if window is None:continue
            subprocess.run(['xdotool','windowactivate','--sync',window,'key','--clearmodifiers','Return'],check=True,timeout=10);self.completed.add(op_id)
    def record_diagnostics(self,response):
        if self.seen!=self.expected:raise OracleProtocolError('incomplete native corner sequence')
