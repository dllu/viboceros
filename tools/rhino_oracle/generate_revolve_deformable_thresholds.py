"""Freeze boundary tests for automatic profile-domain rescaling."""
from .capture_family import capture
from .revolve_deformable_cases import threshold_cases
if __name__=='__main__':
    capture('revolve_deformable_thresholds',dict(protocol_version=1,iterations=1,
        tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=threshold_cases()),
        ['generate_revolve_deformable_thresholds.py','revolve_deformable_cases.py','revolve_deformable_probe.py','join_probe.py','merge_edges_probe.py','structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOracleRevolveDeformableThresholds20261106',timeout=900)
