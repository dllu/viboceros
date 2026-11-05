"""Freeze public SDK extrusion/fitting diagnostics."""
from .capture_family import capture
from .extrude_curve_subd_cases import diagnostics
if __name__=='__main__':
    capture('extrude_curve_subd_sdk',dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=diagnostics()),
        ['generate_extrude_curve_subd_sdk.py','extrude_curve_subd_sdk_probe.py','extrude_curve_subd_cases.py','revolve_subd_cases.py','revolve_splitting_cases.py','subd_box_probe.py','structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOracleExtrudeCurveSubDSDK20261106',timeout=1200)
