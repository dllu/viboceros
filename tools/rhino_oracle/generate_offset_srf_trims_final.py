"""Repeat postselection with DeleteInput set while its option is available."""
from .capture_family import capture
from .offset_surface_command import request
from .offset_surface_trim_cases import TRIM_FINAL
from .structural_sdk import PROBES

if __name__ == '__main__':
    module,deps = PROBES['offset_surface_command']
    capture('offset_srf_trims_final',request(list(TRIM_FINAL)),[
        'generate_offset_srf_trims_final.py','structural_sdk.py','rhino_worker.py','client.py',
        'catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py','group_picking.py',
        'chamfer_point_picker.py','offset_surface_point_picker.py',
        *(name+'.py' for name in (module,*deps)),
    ],'VibocerosOracleOffsetSrfTrimsFinal20261108',timeout=1200)
