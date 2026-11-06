"""Measure Plane's postselected versus preselected guide lifecycle."""
from .capture_family import capture
from .plane_surface_cases import request, GUIDE_CASES

if __name__ == '__main__':
    capture('plane_surface_guides', request(list(GUIDE_CASES)),
            ['generate_plane_surface_guides.py', 'plane_surface_cases.py',
             'plane_surface_probe.py', 'plane_surface_holdouts.py', 'plane_surface_guide_cases.py',
             'join_probe.py', 'merge_edges_probe.py', 'structural_sdk.py',
             'rhino_worker.py', 'client.py'],
            'VibocerosOraclePlaneGuides20261106', timeout=600)
