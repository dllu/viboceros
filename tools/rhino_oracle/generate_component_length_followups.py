"""Capture preselected mesh and SubD edge lengths in a private Xvfb session."""
from .capture_family import capture
from .component_length_cases import followups
if __name__=='__main__':
    request=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=followups())
    capture('component_length_followups',request,['generate_component_length_followups.py','component_length_cases.py','component_length_probe.py','subd_join_probe.py','subd_join_cases.py','edge_loop_cases.py','client.py','rhino_worker.py','structural_sdk.py','merge_edges_probe.py'],'VibocerosOracleComponentLengthFollowups20261104',timeout=240)
