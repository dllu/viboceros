"""Capture real edge picks and mode, preservation, direction and cancellation MatchSrf workflows."""
from .capture_family import capture
from .match_surface_command import TOLERANCE_DEFAULTS,request
from .structural_sdk import PROBES


if __name__=='__main__':
    module,dependencies=PROBES['match_surface_command']
    capture('match_surface_command_tolerance_defaults',request(list(TOLERANCE_DEFAULTS)),[
        'generate_match_surface_command_tolerance_defaults.py','structural_sdk.py','rhino_worker.py','client.py',
        'catenary_point_picker.py','arc_point_picker.py','subcurve_direction_input.py',
        'group_picking.py','chamfer_point_picker.py','match_surface_point_picker.py',
        *(name+'.py' for name in (module,*dependencies)),
    ],'VibocerosOracleMatchSrfCommandToleranceDefaults20261108',timeout=1200)
