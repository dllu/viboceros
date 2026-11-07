"""Capture initial ordered Fillet picks and the dynamic-radius prompt."""
from .capture_family import capture
from .fillet_command import request
from .structural_sdk import PROBES

if __name__ == '__main__':
    module, deps = PROBES['fillet_command']
    capture('fillet_getter_pilot', request(), [
        'generate_fillet_getter_pilot.py', 'structural_sdk.py', 'rhino_worker.py', 'client.py',
        'catenary_point_picker.py', 'arc_point_picker.py', 'subcurve_direction_input.py', 'group_picking.py',
        *(name + '.py' for name in (module, *deps)),
    ], 'VibocerosOracleFilletGetterPilot20261107', timeout=1200)
