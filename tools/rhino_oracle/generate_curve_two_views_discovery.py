"""Capture Crv2View grammar and independent public SDK output."""
from .capture_family import capture
from .curve_two_views_cases import discovery
if __name__=='__main__':
    capture('curve_two_views_discovery',dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=discovery()),
        ['generate_curve_two_views_discovery.py','curve_two_views_cases.py','curve_two_views_probe.py','join_probe.py','merge_edges_probe.py','structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOracleCurveTwoViewsDiscovery20261106',timeout=1200)
