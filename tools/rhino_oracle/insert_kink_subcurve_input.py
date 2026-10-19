"""The shared owned-window driver acknowledges real pointer movement only."""
from .group_picking import IdlePicker
from .subcurve_direction_input import SubcurveDirectionPicker
from .structural_sdk import validate_operations
class InsertKinkSubcurvePicker(SubcurveDirectionPicker):
    def __init__(self,request):
        validate_operations(request)
        operations=request['operations']
        if not 1<=len(operations)<=128 or any(o['op']!='insert_kink_subcurve'for o in operations):raise ValueError('owned kink input requires a dedicated request')
        if len({o['id']for o in operations})!=len(operations):raise ValueError('duplicate owned kink input')
        IdlePicker.__init__(self)
        self.cases={'@subcurve-direction:'+o['id']:o for o in operations if o['getter'].get('cancel_getter')};self.moved={}

    def send_input(self,name,x,y,window):
        from .client import OracleProtocolError
        import subprocess
        if name not in self.cases:raise OracleProtocolError('unexpected temporary kink input marker')
        if self.cases[name]['getter'].get('cancel_getter'):
            subprocess.run(['xdotool','windowactivate','--sync',window,'key','--clearmodifiers','Escape','Escape'],check=True,timeout=10)
            import time,json
            time.sleep(.5)
            case=self.cases[name]['id']
            if not (self.job/('kink-finished-'+case+'.json')).exists():
                (self.job/('kink-returned-'+case+'.json')).write_text(json.dumps(case))
                subprocess.run(['xdotool','windowactivate','--sync',window,'key','--clearmodifiers','Return'],check=True,timeout=10)
            return True
        return super().send_input(name,x,y,window)

    def record_diagnostics(self,response):
        from .client import OracleProtocolError
        if self.seen!=set(self.cases):raise OracleProtocolError('incomplete owned temporary input')
