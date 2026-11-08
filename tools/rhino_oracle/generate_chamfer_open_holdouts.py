"""Capture independent lengths, direction reversals and curved Chamfer controls."""
from .capture_family import capture
from .chamfer_command import OPEN_HOLDOUTS, request
from .structural_sdk import PROBES

if __name__ == '__main__':
    module,deps=PROBES['chamfer_command']
    capture('chamfer_open_holdouts',request(list(OPEN_HOLDOUTS)),[
        'generate_chamfer_open_holdouts.py','structural_sdk.py','rhino_worker.py','client.py',
        'catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',
        *(name+'.py'for name in(module,*deps)),
    ],'VibocerosOracleChamferOpenHoldouts20261108',timeout=2400)
