"""Confirm edge snapshots while freezing the complete physical input driver."""
from .capture_family import capture
from .curve_boolean_subcurve_cases import edges
if __name__=='__main__':
    capture('curve_boolean_subcurve_edge_receipt',dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=edges()),
        ['generate_curve_boolean_subcurve_edge_receipt.py','curve_boolean_subcurve_cases.py','curve_boolean_cases.py','curve_boolean_probe.py',
         'curve_boolean_edge_input.py','owned_brep_command.py','component_picking.py','hole_picking.py','group_picking.py',
         'planar_regions_cases.py','join_probe.py','merge_edges_probe.py','structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOracleCurveBooleanSubcurveEdgeReceipt20261106',timeout=1500)
