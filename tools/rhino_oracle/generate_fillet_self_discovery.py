"""Capture accepted same-parent Fillet topology, domains and history."""
from .capture_family import capture
from .fillet_command import SELF_DISCOVERY, request
from .structural_sdk import PROBES

if __name__ == '__main__':
    module, deps = PROBES['fillet_command']
    capture('fillet_self_discovery', request(list(SELF_DISCOVERY)), [
        'generate_fillet_self_discovery.py', 'structural_sdk.py', 'rhino_worker.py', 'client.py',
        'catenary_point_picker.py', 'arc_point_picker.py', 'subcurve_direction_input.py', 'group_picking.py',
        *(name + '.py' for name in (module, *deps)),
    ], 'VibocerosOracleFilletSelfDiscovery20261107', timeout=1500)
