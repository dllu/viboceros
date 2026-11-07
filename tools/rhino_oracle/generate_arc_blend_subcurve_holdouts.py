"""Hold out closed, reversed, shared-parent, numeric and locked SubCrv inputs."""
from .capture_family import capture
from .arc_blend_command import request,SUBCURVE_HOLDOUTS
from .structural_sdk import PROBES
if __name__=='__main__':
    module,deps=PROBES['arc_blend_command']
    capture('arc_blend_subcurve_holdouts',request(list(SUBCURVE_HOLDOUTS)),[
        'generate_arc_blend_subcurve_holdouts.py','structural_sdk.py','rhino_worker.py',
        'client.py','catenary_point_picker.py','arc_point_picker.py',
        'subcurve_direction_input.py','group_picking.py',
        *(name+'.py'for name in(module,*deps)),
    ],'VibocerosOracleArcBlendSubcurveHoldouts20261107',timeout=1500)
