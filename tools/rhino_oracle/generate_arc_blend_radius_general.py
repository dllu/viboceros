"""Capture bounded RadiusDifference targets through private Rhino input."""
from .capture_family import capture
from .arc_blend_command import request, RADIUS_GENERAL
from .structural_sdk import PROBES

if __name__ == '__main__':
    module, deps = PROBES['arc_blend_command']
    capture('arc_blend_radius_general', request(list(RADIUS_GENERAL)), [
        'generate_arc_blend_radius_general.py', 'structural_sdk.py', 'rhino_worker.py',
        'client.py', 'catenary_point_picker.py', 'arc_point_picker.py',
        'subcurve_direction_input.py', 'group_picking.py',
        *(name + '.py' for name in (module, *deps)),
    ], 'VibocerosOracleArcBlendRadiusGeneral20261106', timeout=1200)
