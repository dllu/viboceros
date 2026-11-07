"""Observe same-parent Fillet pick acceptance through private public getters."""
from .capture_family import capture
from .fillet_command import SELF_PILOT, request
from .structural_sdk import PROBES

if __name__ == '__main__':
    module, deps = PROBES['fillet_command']
    capture('fillet_self_pilot', request(list(SELF_PILOT)), [
        'generate_fillet_self_pilot.py', 'structural_sdk.py', 'rhino_worker.py', 'client.py',
        'catenary_point_picker.py', 'arc_point_picker.py', 'subcurve_direction_input.py', 'group_picking.py',
        *(name + '.py' for name in (module, *deps)),
    ], 'VibocerosOracleFilletSelfPilot20261107', timeout=600)
