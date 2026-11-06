"""Observe Plane Enter completion and assignment-form mode tokens."""
from .capture_family import capture
from .plane_surface_cases import request, FINISH_DISCOVERY

if __name__ == '__main__':
    capture('plane_finish_discovery', request(list(FINISH_DISCOVERY)),
            ['generate_plane_finish_discovery.py', 'plane_finish_cases.py',
             'plane_surface_cases.py', 'plane_surface_probe.py',
             'plane_surface_holdouts.py', 'plane_surface_guide_cases.py',
             'conic_getter_input.py', 'managed_mouse_callback.py',
             'snap_environment.py', 'number_token.py', 'join_probe.py',
             'merge_edges_probe.py', 'structural_sdk.py', 'rhino_worker.py', 'client.py'],
            'VibocerosOraclePlaneFinishDiscovery20261106', timeout=900)
