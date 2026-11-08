"""Capture opposite-seam picks without curved-source extension on private Xvfb."""
from .capture_family import capture
from .chamfer_command import BRANCH_SEAMS,request
from .structural_sdk import PROBES
if __name__=='__main__':
    module,deps=PROBES['chamfer_command']
    capture('chamfer_branch_seams',request(list(BRANCH_SEAMS)),[
        'generate_chamfer_branch_seams.py','structural_sdk.py','rhino_worker.py','client.py',
        'chamfer_point_picker.py','catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',
        *(name+'.py'for name in (module,*deps)),
    ],'VibocerosOracleChamferBranchSeams20261108',timeout=1200)
