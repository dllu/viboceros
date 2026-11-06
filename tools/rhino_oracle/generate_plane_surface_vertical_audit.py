"""Separate native Vertical point constraints from deformable surface output."""
from .capture_family import capture
from .plane_surface_cases import request, VERTICAL_AUDIT

if __name__ == '__main__':
    capture('plane_surface_vertical_audit', request(VERTICAL_AUDIT),
            ['generate_plane_surface_vertical_audit.py', 'plane_surface_cases.py',
             'plane_surface_probe.py', 'plane_surface_holdouts.py', 'plane_surface_guide_cases.py',
             'join_probe.py', 'merge_edges_probe.py', 'structural_sdk.py',
             'rhino_worker.py', 'client.py'],
            'VibocerosOraclePlaneVerticalAudit20261106', timeout=600)
