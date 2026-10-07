"""Reuse the owned hover acknowledgement for the closed numeric matrix."""
from .group_picking import IdlePicker
from .subcurve_direction_input import SubcurveDirectionPicker
from .subcurve_direction_grid_probe import validate_request
class DirectionGridPicker(SubcurveDirectionPicker):
    def __init__(self,request):
        validate_request(request);IdlePicker.__init__(self)
        self.cases={'@subcurve-direction:'+o['id']:o for o in request['operations']};self.moved={}
