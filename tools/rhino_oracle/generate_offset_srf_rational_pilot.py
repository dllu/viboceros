"""Capture signed, two-sided and fine rational curvature-break offsets."""
from .capture_family import capture
from .offset_surface_command import request
from .offset_surface_rational_cases import RATIONAL_PILOT
from .structural_sdk import PROBES

if __name__ == '__main__':
    module,deps = PROBES['offset_surface_command']
    capture('offset_srf_rational_pilot',request(list(RATIONAL_PILOT)),[
        'generate_offset_srf_rational_pilot.py','structural_sdk.py','rhino_worker.py','client.py',
        'catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',
        'chamfer_point_picker.py','offset_surface_point_picker.py',
        *(name+'.py' for name in (module,*deps)),
    ],'VibocerosOracleOffsetSrfRationalPilot20261108',timeout=1200)
