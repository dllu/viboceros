"""Capture reversed sources and noncircular-boundary end choices on private Xvfb."""
from .capture_family import capture
from .chamfer_command import BRANCH_HOLDOUTS,request
from .structural_sdk import PROBES
if __name__=='__main__':
    module,deps=PROBES['chamfer_command']
    capture('chamfer_branch_holdouts',request(list(BRANCH_HOLDOUTS)),[
        'generate_chamfer_branch_holdouts.py','structural_sdk.py','rhino_worker.py','client.py',
        'chamfer_point_picker.py','catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',
        *(name+'.py'for name in (module,*deps)),
    ],'VibocerosOracleChamferBranchHoldouts20261108',timeout=1200)
