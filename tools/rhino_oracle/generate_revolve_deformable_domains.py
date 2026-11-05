"""Probe source-domain policy and profile-axis contacts independently."""
from .capture_family import capture
from .revolve_deformable_cases import domain_cases
if __name__=='__main__':
    capture('revolve_deformable_domains',dict(protocol_version=1,iterations=1,
        tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=domain_cases()),
        ['generate_revolve_deformable_domains.py','revolve_deformable_cases.py','revolve_deformable_probe.py','join_probe.py','merge_edges_probe.py','structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOracleRevolveDeformableDomains20261106',timeout=900)
