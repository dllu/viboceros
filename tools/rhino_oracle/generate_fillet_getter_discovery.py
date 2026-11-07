"""Capture complete fixed-radius picks and discover dynamic radius input."""
from .capture_family import capture
from .fillet_command import request, DISCOVERY
from .structural_sdk import PROBES

if __name__ == '__main__':
    module, deps = PROBES['fillet_command']
    capture('fillet_getter_discovery', request(list(DISCOVERY)), [
        'generate_fillet_getter_discovery.py', 'structural_sdk.py', 'rhino_worker.py', 'client.py',
        'catenary_point_picker.py', 'arc_point_picker.py', 'subcurve_direction_input.py', 'group_picking.py',
        *(name + '.py' for name in (module, *deps)),
    ], 'VibocerosOracleFilletGetterDiscovery20261107', timeout=1200)
