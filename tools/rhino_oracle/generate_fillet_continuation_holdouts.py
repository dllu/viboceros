"""Capture curved-source Fillet orientation, disjoint supports and radius controls."""
from .capture_family import capture
from .fillet_command import CONTINUATION_HOLDOUTS,request
from .structural_sdk import PROBES
if __name__=='__main__':
    module,deps=PROBES['fillet_command']
    capture('fillet_continuation_holdouts',request(list(CONTINUATION_HOLDOUTS)),[
        'generate_fillet_continuation_holdouts.py','structural_sdk.py','rhino_worker.py','client.py',
        'catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',
        *(name+'.py'for name in (module,*deps)),
    ],'VibocerosOracleFilletContinuationHoldouts20261108',timeout=1500)
