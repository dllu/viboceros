"""Closed shells and the seam-wall policy of closed-parameter sheets."""
from .capture_family import capture
from .offset_surface_command import request
from .offset_surface_loose_cases import LOOSE_CLOSED
from .structural_sdk import PROBES

if __name__=='__main__':
    module,deps=PROBES['offset_surface_command']
    capture('offset_srf_loose_closed',request(list(LOOSE_CLOSED)),[
        'generate_offset_srf_loose_closed.py','structural_sdk.py','rhino_worker.py','client.py',
        'catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',
        'chamfer_point_picker.py','offset_surface_point_picker.py',
        *(name+'.py'for name in(module,*deps)),
    ],'VibocerosOracleOffsetSrfLooseClosed20261108',timeout=1200)
