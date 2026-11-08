"""Verify Smooth overlap stages and reversed curved consumption."""
from .capture_family import capture
from .chamfer_command import OPEN_CHOICES, request
from .structural_sdk import PROBES

if __name__ == '__main__':
    module,deps=PROBES['chamfer_command']
    capture('chamfer_open_choices',request(list(OPEN_CHOICES)),[
        'generate_chamfer_open_choices.py','structural_sdk.py','rhino_worker.py','client.py',
        'catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',
        *(name+'.py'for name in(module,*deps)),
    ],'VibocerosOracleChamferOpenChoices20261108',timeout=1800)
