"""Freeze independently owned G0 SubD patch controls on a private display."""
from .capture_family import capture
from .subd_g0_probe import cases

if __name__=='__main__':
    query=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=cases())
    capture('subd_to_nurbs_g0',query,['generate_subd_g0.py','subd_g0_probe.py','subd_to_nurbs_probe.py',
        'subd_join_probe.py','subd_join_cases.py','subd_box_probe.py','client.py','rhino_worker.py',
        'structural_sdk.py','merge_edges_probe.py'],'VibocerosOracleSubDG020261104',timeout=1800)
