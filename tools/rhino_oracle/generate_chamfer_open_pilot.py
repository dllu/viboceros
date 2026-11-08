"""Capture open Chamfer contacts privately with actual UndoRedo events."""
from .capture_family import capture
from .chamfer_command import OPEN_PILOT, request
from .structural_sdk import PROBES

if __name__ == '__main__':
    module,deps=PROBES['chamfer_command']
    capture('chamfer_open_pilot',request(list(OPEN_PILOT)),[
        'generate_chamfer_open_pilot.py','structural_sdk.py','rhino_worker.py','client.py',
        'catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',
        *(name+'.py'for name in(module,*deps)),
    ],'VibocerosOracleChamferOpenPilot20261108',timeout=1800)
