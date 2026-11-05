"""Freeze an independent first native SubD straight-extrusion sample."""
from .capture_family import capture
from .extrude_curve_subd_cases import pilot
if __name__=='__main__':
    capture('extrude_curve_subd_pilot',dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=pilot()),
        ['generate_extrude_curve_subd_pilot.py','extrude_curve_subd_cases.py','revolve_subd_cases.py','revolve_splitting_cases.py','extrude_curve_subd_probe.py','subd_box_probe.py','join_probe.py','merge_edges_probe.py','structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOracleExtrudeCurveSubDPilot20261106',timeout=1200)
