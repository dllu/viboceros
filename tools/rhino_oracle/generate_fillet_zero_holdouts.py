"""Capture zero-radius boundary, source-order and continuation controls privately."""
from .capture_family import capture
from .fillet_command import ZERO_HOLDOUTS, request
from .structural_sdk import PROBES

if __name__ == '__main__':
    module, deps = PROBES['fillet_command']
    capture('fillet_zero_holdouts', request(list(ZERO_HOLDOUTS)), [
        'generate_fillet_zero_holdouts.py', 'structural_sdk.py', 'rhino_worker.py', 'client.py',
        'catenary_point_picker.py', 'arc_point_picker.py', 'subcurve_direction_input.py', 'group_picking.py',
        *(name+'.py' for name in (module,*deps)),
    ], 'VibocerosOracleFilletZeroHoldouts20261108', timeout=1800)
