"""Capture owned nested chain getters under private Xvfb only."""
from .capture_family import capture
from .length_chain_cases import cases
if __name__=='__main__':
    request=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=cases())
    capture('length_chain_discovery',request,['generate_length_chain_discovery.py','length_chain_probe.py','length_chain_cases.py','chain_selection_probe.py','chain_selection_cases.py','chain_selection_driver.py','chain_selection_input.py','shrink_face_input.py','component_picking.py','client.py','rhino_worker.py','structural_sdk.py','merge_edges_probe.py'],'VibocerosOracleLengthChainDiscovery20261104',timeout=180)
