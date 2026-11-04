"""Capture loop boundaries, creases, cycles and non-quad controls in private Xvfb."""
from .capture_family import capture
from .edge_loop_cases import cases
if __name__=='__main__':
    query=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=cases())
    capture('edge_loop_commands',query,['generate_edge_loop_commands.py','edge_loop_probe.py','edge_loop_cases.py',
        'subd_join_probe.py','subd_join_cases.py','shrink_face_input.py','client.py',
        'component_picking.py','rhino_worker.py','structural_sdk.py','merge_edges_probe.py'],
        'VibocerosOracleEdgeLoopCommands20261105',timeout=600)
