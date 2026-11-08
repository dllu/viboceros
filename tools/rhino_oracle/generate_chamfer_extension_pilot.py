"""Capture prescribed nonmeeting curved-source Chamfer workflows on private Xvfb."""
from .capture_family import capture
from .chamfer_command import EXTENSION_PILOT,request
from .structural_sdk import PROBES
if __name__=='__main__':
    module,deps=PROBES['chamfer_command']
    capture('chamfer_extension_pilot',request(list(EXTENSION_PILOT)),[
        'generate_chamfer_extension_pilot.py','structural_sdk.py','rhino_worker.py','client.py',
        'chamfer_point_picker.py','catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',
        *(name+'.py' for name in (module,*deps)),
    ],'VibocerosOracleChamferExtensionPilot20261107',timeout=1200)
