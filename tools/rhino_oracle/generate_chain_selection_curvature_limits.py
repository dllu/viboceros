"""Separate radius- and model-scale controls for SelChain curvature tolerance."""
from .capture_family import capture
from .chain_selection_cases import curvature_limit_cases
if __name__=='__main__':
    request=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=curvature_limit_cases())
    capture('chain_selection_curvature_limits',request,['generate_chain_selection_curvature_limits.py','chain_selection_probe.py','chain_selection_cases.py','chain_selection_input.py','chain_selection_driver.py','shrink_face_input.py','component_picking.py','client.py','rhino_worker.py','structural_sdk.py','merge_edges_probe.py'],'VibocerosOracleChainSelectionCurvatureLimits20261104',timeout=240)
