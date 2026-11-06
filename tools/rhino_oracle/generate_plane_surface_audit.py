"""Record Plane degree/count admission and integer getter defaults."""
from .capture_family import capture
from .plane_surface_cases import request, AUDIT

if __name__ == '__main__':
    capture('plane_surface_audit', request(AUDIT),
            ['generate_plane_surface_audit.py', 'plane_surface_cases.py',
             'plane_surface_probe.py', 'plane_surface_guide_cases.py', 'plane_surface_holdouts.py', 'join_probe.py', 'merge_edges_probe.py',
             'structural_sdk.py', 'rhino_worker.py', 'client.py'],
            'VibocerosOraclePlaneAudit20261106', timeout=600)
