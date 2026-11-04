"""Capture mixed component preselection in private Xvfb with private preferences."""
from .capture_family import capture
from .component_pattern_cases import cases
if __name__=='__main__':
    query=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=cases())
    capture('component_pattern_preselection',query,['generate_component_pattern_preselection.py','component_pattern_cases.py','component_pattern_probe.py','edge_ring_cases.py','edge_loop_workflow_probe.py',
        'edge_loop_probe.py','edge_loop_cases.py','subd_join_probe.py','subd_join_cases.py','shrink_face_input.py',
        'component_picking.py','client.py','rhino_worker.py','structural_sdk.py','merge_edges_probe.py'],
        'VibocerosOracleComponentPatternPreselection20261105',timeout=600)
