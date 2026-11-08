"""Capture OffsetSrf's distance, option and selection stages privately."""
from .capture_family import capture
from .offset_surface_command import HOLDOUTS, request
from .structural_sdk import PROBES

if __name__ == '__main__':
    module,deps=PROBES['offset_surface_command']
    capture('offset_srf_getter_holdouts',request(list(HOLDOUTS)),[
        'generate_offset_srf_getter_holdouts.py','structural_sdk.py','rhino_worker.py','client.py',
        'catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',
        'chamfer_point_picker.py','offset_surface_point_picker.py',
        *(name+'.py'for name in(module,*deps)),
    ],'VibocerosOracleOffsetSrfGetterHoldouts20261108',timeout=1200)
