"""Native pointer-constrained temporary sources, unit lengths and nested cancel."""
from .capture_family import capture
from .arc_blend_command import request,SUBCURVE_POINTERS
from .structural_sdk import PROBES
if __name__=='__main__':
    module,deps=PROBES['arc_blend_command']
    capture('arc_blend_subcurve_pointers',request(list(SUBCURVE_POINTERS)),[
        'generate_arc_blend_subcurve_pointers.py','structural_sdk.py','rhino_worker.py',
        'client.py','catenary_point_picker.py','arc_point_picker.py',
        'subcurve_direction_input.py','group_picking.py',
        *(name+'.py'for name in(module,*deps)),
    ],'VibocerosOracleArcBlendSubcurvePointers20261107',timeout=1200)
