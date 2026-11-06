"""Record Plane modes, deformable bases, cancellation and document history."""
from .capture_family import capture
from .plane_surface_cases import request

if __name__ == '__main__':
    capture('plane_surface_workflows', request(),
            ['generate_plane_surface_workflows.py', 'plane_surface_cases.py',
             'plane_surface_probe.py', 'plane_surface_guide_cases.py', 'plane_surface_holdouts.py', 'join_probe.py', 'merge_edges_probe.py',
             'structural_sdk.py', 'rhino_worker.py', 'client.py'],
            'VibocerosOraclePlaneWorkflows20261106', timeout=900)
