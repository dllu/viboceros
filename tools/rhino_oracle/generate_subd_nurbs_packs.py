"""Freeze native face-pack action toggles and lifecycle on private Xvfb."""
from .capture_family import capture
from .subd_nurbs_packs_probe import cases
if __name__=='__main__':
    query=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=cases())
    capture('subd_nurbs_packs',query,['generate_subd_nurbs_packs.py','subd_nurbs_packs_probe.py',
        'join_probe.py','merge_edges_probe.py','client.py','rhino_worker.py','structural_sdk.py'],
        'VibocerosOracleSubDNurbsPacksHoldouts20261105',timeout=1800)
