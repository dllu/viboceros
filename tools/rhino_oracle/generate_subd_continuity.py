"""Freeze public SubD continuity modes with immutable source recipes."""
from .capture_family import capture
from .subd_continuity_probe import cases

if __name__=='__main__':
    query=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=cases())
    capture('subd_continuity',query,['generate_subd_continuity.py','subd_continuity_probe.py',
        'subd_g0_probe.py','subd_to_nurbs_probe.py','subd_join_probe.py','subd_join_cases.py',
        'subd_box_probe.py','client.py','rhino_worker.py','structural_sdk.py','merge_edges_probe.py'],
        'VibocerosOracleSubDContinuity20261104',timeout=1800)
