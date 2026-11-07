"""Capture Chamfer distance continuation, zero cases and curved sources."""
from .capture_family import capture
from .chamfer_command import DISCOVERY, request
from .structural_sdk import PROBES

if __name__ == '__main__':
    module, deps = PROBES['chamfer_command']
    capture('chamfer_getter_discovery', request(list(DISCOVERY)), [
        'generate_chamfer_getter_discovery.py', 'structural_sdk.py', 'rhino_worker.py', 'client.py',
        'chamfer_point_picker.py', 'catenary_point_picker.py', 'arc_point_picker.py', 'subcurve_direction_input.py', 'group_picking.py',
        *(name + '.py' for name in (module, *deps)),
    ], 'VibocerosOracleChamferGetterDiscovery20261107', timeout=1500)
