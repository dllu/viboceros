"""Freeze nested display changes in private Xvfb."""
from .capture_family import capture
from .nurbs_edge_command_probe import interaction_cases,ASSETS
if __name__=='__main__':
    query=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=interaction_cases())
    capture('nurbs_edge_nested',query,['generate_nurbs_edge_nested.py','nurbs_edge_command_probe.py',
        'subd_packing_cases.py','subd_g1x_probe.py','subd_continuity_probe.py','subd_g0_probe.py','subd_to_nurbs_probe.py',
        'subd_join_probe.py','subd_join_cases.py','subd_box_probe.py','join_probe.py','client.py','rhino_worker.py',
        'structural_sdk.py','merge_edges_probe.py']+
        ['fixtures/subd_soft_nurbs_inputs/'+a+'.3dm' for a in ASSETS],
        'VibocerosOracleNurbsEdgeInteractions20261105',timeout=1800)
