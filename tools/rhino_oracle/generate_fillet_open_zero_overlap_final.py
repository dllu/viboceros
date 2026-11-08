"""Repeat every collinear control with separated long-line physical picks."""
from .capture_family import capture
from .fillet_command import OPEN_ZERO_OVERLAP_FINAL, request
from .structural_sdk import PROBES

if __name__ == '__main__':
    module,deps=PROBES['fillet_command']
    capture('fillet_open_zero_overlap_final',request(list(OPEN_ZERO_OVERLAP_FINAL)),[
        'generate_fillet_open_zero_overlap_final.py','structural_sdk.py','rhino_worker.py','client.py',
        'catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',
        *(name+'.py'for name in(module,*deps)),
    ],'VibocerosOracleFilletOpenZeroOverlapFinal20261108',timeout=1800)
