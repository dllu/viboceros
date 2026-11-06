"""Independent scaled and remote Plane constructions on oblique CPlanes."""
from .capture_family import capture
from .plane_surface_cases import request, HOLDOUTS

if __name__ == '__main__':
    capture('plane_surface_holdouts', request(HOLDOUTS),
            ['generate_plane_surface_holdouts.py', 'plane_surface_cases.py',
             'plane_surface_probe.py', 'plane_surface_guide_cases.py', 'plane_surface_holdouts.py', 'join_probe.py', 'merge_edges_probe.py',
             'structural_sdk.py', 'rhino_worker.py', 'client.py'],
            'VibocerosOraclePlaneHoldouts20261106', timeout=600)
