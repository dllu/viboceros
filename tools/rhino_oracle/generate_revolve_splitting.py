"""Freeze public split policy observations without desktop windows."""
from .capture_family import capture
from .revolve_splitting_cases import cases
if __name__=='__main__':
    capture('revolve_splitting',dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=cases()),
        ['generate_revolve_splitting.py','revolve_splitting_cases.py','revolve_splitting_probe.py','join_probe.py','merge_edges_probe.py','structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOracleRevolveSplitting20261106',timeout=1200)
