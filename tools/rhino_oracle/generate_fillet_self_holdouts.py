"""Capture same-parent seam, source-order, representation and failure holdouts."""
from .capture_family import capture
from .fillet_command import SELF_HOLDOUTS, request
from .structural_sdk import PROBES

if __name__ == '__main__':
    module, deps = PROBES['fillet_command']
    capture('fillet_self_holdouts', request(list(SELF_HOLDOUTS)), [
        'generate_fillet_self_holdouts.py', 'structural_sdk.py', 'rhino_worker.py', 'client.py',
        'catenary_point_picker.py', 'arc_point_picker.py', 'subcurve_direction_input.py', 'group_picking.py',
        *(name + '.py' for name in (module, *deps)),
    ], 'VibocerosOracleFilletSelfHoldouts20261107', timeout=1500)
