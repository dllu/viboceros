"""Shared acknowledged hover and click delivery in an owned Arc viewport."""
from .catenary_point_picker import CatenaryPointPicker
from .subcurve_direction_input import SubcurveDirectionPicker

class ArcPointPicker(CatenaryPointPicker):
    def __init__(self,request):
        super().__init__(request)
        self.hovers={'@subcurve-direction:'+op['id']+'-'+str(i):dict(id=op['id']+'-'+str(i))for op in request['operations']for i,a in enumerate(op['actions'])if isinstance(a,dict)and'hover'in a}
        self.cases.update(self.hovers)
    def send_input(self,name,x,y,window):
        if name in self.hovers:return SubcurveDirectionPicker.send_input(self,name,x,y,window)
        return super().send_input(name,x,y,window)
