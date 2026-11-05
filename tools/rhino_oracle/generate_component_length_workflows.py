"""Capture command-first edge picking and cleanup under private Xvfb."""
from .capture_family import capture
from .component_length_cases import workflows
if __name__=='__main__':
    request=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=workflows())
    capture('component_length_workflows',request,['generate_component_length_workflows.py','component_length_cases.py','component_length_probe.py','subd_join_probe.py','subd_join_cases.py','edge_loop_cases.py','client.py','rhino_worker.py','structural_sdk.py','merge_edges_probe.py','shrink_face_input.py','chain_selection_driver.py','component_picking.py'],'VibocerosOracleComponentLengthWorkflows20261104',timeout=240)
