"""Capture owned SubD area commands exclusively through the headless wrapper."""
from .capture_family import capture
from .subd_area_cases import followups
if __name__=='__main__':
    request=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=followups())
    capture('subd_area_followups',request,['generate_subd_area_followups.py','subd_area_cases.py','subd_area_probe.py','component_length_cases.py','subd_to_nurbs_probe.py','subd_box_probe.py','subd_join_probe.py','subd_join_cases.py','edge_loop_cases.py','client.py','rhino_worker.py','structural_sdk.py','join_probe.py','merge_edges_probe.py'],'VibocerosOracleSubDAreaFollowups20261104',timeout=360)
