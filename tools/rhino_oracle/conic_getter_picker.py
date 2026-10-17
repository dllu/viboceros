"""Reuse owned SubCrv hover delivery and acknowledgements for Conic getters."""
from .group_picking import IdlePicker
from .subcurve_direction_input import SubcurveDirectionPicker
class ConicGetterPicker(SubcurveDirectionPicker):
    def __init__(self,request):
        IdlePicker.__init__(self)
        self.cases={'@subcurve-direction:'+op['id']+'-'+str(i):dict(id=op['id']+'-'+str(i)) for op in request['operations'] for i,a in enumerate(op['actions'])if isinstance(a,dict)and'hover'in a}
        self.moved={}

        self.edges={'@conic-edge:'+op['id']+'-'+str(i)for op in request['operations']for i,a in enumerate(op['actions'])if isinstance(a,dict)and'edge'in a}
    def send_input(self,name,x,y,window):
        if name in self.edges:return IdlePicker.send_input(self,name,x,y,window)
        return super().send_input(name,x,y,window)
    def record_diagnostics(self,response):
        from .client import OracleProtocolError
        if self.seen!=set(self.cases)|self.edges:raise OracleProtocolError('incomplete owned Conic input')
