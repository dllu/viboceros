"""Capture open Chamfer curve contacts, complete consumption and joining."""
from .capture_family import capture
from .chamfer_command import OPEN_CURVES, request
from .structural_sdk import PROBES

if __name__ == '__main__':
    module,deps=PROBES['chamfer_command']
    capture('chamfer_open_curves',request(list(OPEN_CURVES)),[
        'generate_chamfer_open_curves.py','structural_sdk.py','rhino_worker.py','client.py',
        'catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',
        *(name+'.py'for name in(module,*deps)),
    ],'VibocerosOracleChamferOpenCurves20261108',timeout=2700)
