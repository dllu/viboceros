"""Capture smooth Fillet radius and source-span controls on private Xvfb."""
from .capture_family import capture
from .fillet_command import CONTINUATION_VERIFICATION,request
from .structural_sdk import PROBES
if __name__=='__main__':
    module,deps=PROBES['fillet_command']
    capture('fillet_continuation_verification',request(list(CONTINUATION_VERIFICATION)),[
        'generate_fillet_continuation_verification.py','structural_sdk.py','rhino_worker.py','client.py',
        'catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',
        *(name+'.py'for name in (module,*deps)),
    ],'VibocerosOracleFilletContinuationVerification20261108',timeout=1500)
