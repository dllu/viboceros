"""Capture zero-radius Fillet getters on a private Xvfb display."""
from .capture_family import capture
from .fillet_command import ZERO_PILOT, request
from .structural_sdk import PROBES

if __name__ == '__main__':
    module, deps = PROBES['fillet_command']
    capture('fillet_zero_pilot', request(list(ZERO_PILOT)), [
        'generate_fillet_zero_pilot.py', 'structural_sdk.py', 'rhino_worker.py', 'client.py',
        'catenary_point_picker.py', 'arc_point_picker.py', 'subcurve_direction_input.py', 'group_picking.py',
        *(name+'.py' for name in (module,*deps)),
    ], 'VibocerosOracleFilletZeroPilot20261108', timeout=1500)
