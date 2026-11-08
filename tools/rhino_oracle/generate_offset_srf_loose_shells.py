"""Prime hidden deletion preferences before closed-shell admissions."""
from .capture_family import capture
from .offset_surface_command import request
from .offset_surface_loose_cases import LOOSE_SHELLS
from .structural_sdk import PROBES

if __name__=='__main__':
    module,deps=PROBES['offset_surface_command']
    capture('offset_srf_loose_shells',request(list(LOOSE_SHELLS)),[
        'generate_offset_srf_loose_shells.py','structural_sdk.py','rhino_worker.py','client.py',
        'catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',
        'chamfer_point_picker.py','offset_surface_point_picker.py',
        *(name+'.py'for name in(module,*deps)),
    ],'VibocerosOracleOffsetSrfLooseShells20261108',timeout=1200)
