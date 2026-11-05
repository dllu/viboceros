"""Capture partial turns, negative turns and separate angular thresholds."""
from .capture_family import capture
from .revolve_splitting_cases import holdouts
if __name__=='__main__':
    capture('revolve_splitting_holdouts',dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=holdouts()),
        ['generate_revolve_splitting_holdouts.py','revolve_splitting_cases.py','revolve_splitting_probe.py','join_probe.py','merge_edges_probe.py','structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOracleRevolveSplittingHoldouts20261106',timeout=1200)
