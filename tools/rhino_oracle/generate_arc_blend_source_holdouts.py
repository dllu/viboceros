"""Hold out native mouse contacts and arithmetic with length suffixes."""
from .capture_family import capture
from .arc_blend_command import request,SOURCE_INPUT_HOLDOUTS
from .structural_sdk import PROBES

if __name__=='__main__':
    module,deps=PROBES['arc_blend_command']
    capture('arc_blend_source_holdouts',request(list(SOURCE_INPUT_HOLDOUTS)),[
        'generate_arc_blend_source_holdouts.py','structural_sdk.py','rhino_worker.py',
        'client.py','catenary_point_picker.py','arc_point_picker.py',
        'subcurve_direction_input.py','group_picking.py',
        *(name+'.py'for name in(module,*deps)),
    ],'VibocerosOracleArcBlendSourceHoldouts20261106',timeout=1200)
