"""Capture profile reconstruction, direction, temporary input and cancellation."""
from .capture_family import capture
from .curve_two_views_cases import cases
if __name__=='__main__':
    capture('curve_two_views',dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=cases()),
        ['generate_curve_two_views.py','curve_two_views_cases.py','curve_two_views_probe.py','planar_regions_cases.py','join_probe.py','merge_edges_probe.py','structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOracleCurveTwoViews20261106',timeout=1800)
