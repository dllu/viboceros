"""Observe preselection acceptance and command-first MergeSrf getters."""
from .capture_family import capture
from .merge_surface_command import DISCOVERY, request
from .structural_sdk import PROBES


if __name__ == '__main__':
    module, dependencies = PROBES['merge_surface_command']
    capture('merge_surface_command_discovery', request(list(DISCOVERY)), [
        'generate_merge_surface_command_discovery.py', 'structural_sdk.py', 'rhino_worker.py', 'client.py',
        'catenary_point_picker.py', 'arc_point_picker.py', 'subcurve_direction_input.py',
        'group_picking.py', 'chamfer_point_picker.py', 'merge_surface_point_picker.py',
        *(name + '.py' for name in (module, *dependencies)),
    ], 'VibocerosOracleMergeSrfCommandDiscovery20261108', timeout=1200)
