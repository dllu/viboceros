"""Capture count limits, active domains, closure, spatial axes and scales."""
from .capture_family import capture
from .revolve_subd_cases import edge_cases
if __name__=='__main__':
    capture('revolve_subd_edges',dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=edge_cases()),
        ['generate_revolve_subd_edges.py','revolve_subd_cases.py','revolve_splitting_cases.py','revolve_subd_probe.py','subd_box_probe.py','join_probe.py','merge_edges_probe.py','structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOracleRevolveSubDEdges20261106',timeout=1200)
