"""Capture actual SubD Flip/Dir commands under the private-display wrapper."""
from .capture_family import capture
from .subd_direction_cases import cases
if __name__=='__main__':
    request=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=cases())
    capture('subd_direction_discovery',request,['generate_subd_direction_discovery.py','subd_direction_cases.py','subd_direction_probe.py','subd_volume_cases.py','subd_area_cases.py','component_length_cases.py','subd_to_nurbs_probe.py','subd_box_probe.py','subd_join_probe.py','subd_join_cases.py','edge_loop_cases.py','client.py','rhino_worker.py','structural_sdk.py','join_probe.py','merge_edges_probe.py'],'VibocerosOracleSubDDirectionDiscovery20261105',timeout=600)
