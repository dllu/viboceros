"""Capture inline surface-edge ranges without editing their parent B-reps."""
from .capture_family import capture
from .curve_boolean_subcurve_cases import edges
if __name__=='__main__':
    capture('curve_boolean_subcurve_edges',dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=edges()),
        ['generate_curve_boolean_subcurve_edges.py','curve_boolean_subcurve_cases.py','curve_boolean_cases.py','curve_boolean_probe.py','planar_regions_cases.py','join_probe.py','merge_edges_probe.py','structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOracleCurveBooleanSubcurveEdges20261106',timeout=1500)
