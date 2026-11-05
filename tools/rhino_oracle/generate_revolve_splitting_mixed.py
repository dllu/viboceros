"""Independent mixed continuity breaks establish partition order and sharing."""
from .capture_family import capture
from .revolve_splitting_cases import mixed_cases
if __name__=='__main__':
    capture('revolve_splitting_mixed',dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=mixed_cases()),
        ['generate_revolve_splitting_mixed.py','revolve_splitting_cases.py','revolve_splitting_probe.py','join_probe.py','merge_edges_probe.py','structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOracleRevolveSplittingMixed20261106',timeout=1200)
