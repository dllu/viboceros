"""Capture independent fitted-profile hypotheses at three source scales."""
from .capture_family import capture
from .curve_two_views_cases import fit_audit_cases
if __name__=='__main__':
    capture('curve_two_views_fit_audit',dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=fit_audit_cases()),
        ['generate_curve_two_views_fit_audit.py','curve_two_views_cases.py','curve_two_views_probe.py','curve_two_views_fit_probe.py','planar_regions_cases.py','join_probe.py','merge_edges_probe.py','structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOracleCurveTwoViewsFitAudit20261106',timeout=1200)
