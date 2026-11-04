"""Freeze partial n-gon strips and mixed nonmanifold seeds in private Xvfb."""
from .capture_family import capture
from .component_pattern_cases import nonmanifold_cases
from .face_loop_cases import ngon_cases
if __name__=='__main__':
    query=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=ngon_cases()+nonmanifold_cases())
    capture('component_pattern_holdouts',query,['generate_component_pattern_holdouts.py','component_pattern_cases.py','component_pattern_probe.py','face_loop_cases.py','face_loop_probe.py','edge_ring_cases.py','edge_loop_workflow_probe.py',
        'edge_loop_probe.py','edge_loop_cases.py','subd_join_probe.py','subd_join_cases.py','shrink_face_input.py',
        'component_picking.py','client.py','rhino_worker.py','structural_sdk.py','merge_edges_probe.py'],
        'VibocerosOracleComponentPatternHoldouts20261105',timeout=600)
