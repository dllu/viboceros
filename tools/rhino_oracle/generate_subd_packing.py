"""Freeze packed/unpacked SubD charts and topology on private Xvfb."""
from .capture_family import capture
from .subd_packing_cases import cases

if __name__=='__main__':
    query=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=cases())
    capture('subd_packing',query,['generate_subd_packing.py','subd_packing_probe.py','subd_packing_cases.py',
        'subd_packing_holdouts.py','subd_g1x_probe.py','subd_continuity_probe.py','subd_g0_probe.py','subd_to_nurbs_probe.py',
        'subd_join_probe.py','subd_join_cases.py','subd_box_probe.py','client.py','rhino_worker.py',
        'structural_sdk.py','merge_edges_probe.py'],'VibocerosOracleSubDPacking20261104',timeout=2400)
