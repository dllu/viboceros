"""Hold out radius defaults, cancelled numeric input and pending redo."""
from .capture_family import capture
from .arc_blend_command import request, PREFERENCE_HOLDOUTS
from .structural_sdk import PROBES

if __name__ == '__main__':
    module, deps = PROBES['arc_blend_command']
    capture('arc_blend_preference_holdouts', request(list(PREFERENCE_HOLDOUTS)), [
        'generate_arc_blend_preference_holdouts.py', 'structural_sdk.py', 'rhino_worker.py',
        'client.py', 'catenary_point_picker.py', 'arc_point_picker.py',
        'subcurve_direction_input.py', 'group_picking.py',
        *(name + '.py' for name in (module, *deps)),
    ], 'VibocerosOracleArcBlendPreferenceHoldouts20261106', timeout=1200)
