"""Deliver only requested edge clicks in the newly owned Rhino window."""
from .group_picking import IdlePicker
from .client import OracleProtocolError

class OrientCurveEdgePicker(IdlePicker):
    def __init__(self,request):
        super().__init__()
        self.expected={'@orient-edge:'+op['id']for op in request['operations']if'edge'in op}
    def send_input(self,name,x,y,window):
        if name not in self.expected:raise OracleProtocolError('unexpected edge orientation input marker')
        return super().send_input(name,x,y,window)
    def record_diagnostics(self,response):
        if self.seen!=self.expected:raise OracleProtocolError('incomplete edge orientation clicks')
