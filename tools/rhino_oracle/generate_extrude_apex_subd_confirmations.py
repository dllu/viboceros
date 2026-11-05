"""Freeze an independent confirmation native SubD apex-extrusion sample."""
from .capture_family import capture
from .extrude_apex_subd_cases import confirmations
if __name__=='__main__':
    capture('extrude_apex_subd_confirmations',dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=confirmations()),
        ['generate_extrude_apex_subd_confirmations.py','extrude_apex_subd_cases.py','revolve_subd_cases.py','revolve_splitting_cases.py','extrude_apex_subd_probe.py','subd_box_probe.py','join_probe.py','merge_edges_probe.py','structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOracleExtrudeApexSubDConfirmations20261106',timeout=1200)
