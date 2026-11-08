"""Capture prescribed nonmeeting curved-source Chamfer workflows on private Xvfb."""
from .capture_family import capture
from .chamfer_command import EXTENSION_STYLING,request
from .structural_sdk import PROBES
if __name__=='__main__':
    module,deps=PROBES['chamfer_command']
    capture('chamfer_extension_styling',request(list(EXTENSION_STYLING)),[
        'generate_chamfer_extension_styling.py','structural_sdk.py','rhino_worker.py','client.py',
        'chamfer_point_picker.py','catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',
        *(name+'.py' for name in (module,*deps)),
    ],'VibocerosOracleChamferExtensionStyling20261107',timeout=1200)
