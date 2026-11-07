"""Repeat all closed-source holdouts with separated projected spatial picks."""
from .capture_family import capture
from .fillet_command import CLOSED_FINAL, request
from .structural_sdk import PROBES

if __name__ == '__main__':
    module, deps = PROBES['fillet_command']
    capture('fillet_closed_final', request(list(CLOSED_FINAL)), [
        'generate_fillet_closed_final.py', 'structural_sdk.py', 'rhino_worker.py', 'client.py',
        'catenary_point_picker.py', 'arc_point_picker.py', 'subcurve_direction_input.py', 'group_picking.py',
        *(name + '.py' for name in (module, *deps)),
    ], 'VibocerosOracleFilletClosedFinal20261107', timeout=1500)
