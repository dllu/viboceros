"""Capture prescribed single-parent Chamfer workflows on private Xvfb."""
from .capture_family import capture
from .chamfer_command import SELF_HOLDOUTS,request
from .structural_sdk import PROBES
if __name__=='__main__':
    module,deps=PROBES['chamfer_command']
    capture('chamfer_self_holdouts',request(list(SELF_HOLDOUTS)),[
        'generate_chamfer_self_holdouts.py','structural_sdk.py','rhino_worker.py','client.py',
        'chamfer_point_picker.py','catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',
        *(name+'.py' for name in (module,*deps)),
    ],'VibocerosOracleChamferSelfHoldouts20261107',timeout=1200)
