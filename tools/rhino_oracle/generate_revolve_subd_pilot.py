"""Freeze a first independent native SubD revolution sample."""
from .capture_family import capture
from .revolve_subd_cases import pilot
if __name__=='__main__':
    capture('revolve_subd_pilot',dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=pilot()),
        ['generate_revolve_subd_pilot.py','revolve_subd_cases.py','revolve_splitting_cases.py','revolve_subd_probe.py','subd_box_probe.py','join_probe.py','merge_edges_probe.py','structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOracleRevolveSubDPilot20261106',timeout=1200)
