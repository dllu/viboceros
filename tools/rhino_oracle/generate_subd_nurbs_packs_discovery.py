"""Freeze native face-pack action discovery on private Xvfb."""
from .capture_family import capture
from .subd_nurbs_packs_probe import discovery_cases as cases
if __name__=='__main__':
    query=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=cases())
    capture('subd_nurbs_packs_discovery',query,['generate_subd_nurbs_packs_discovery.py','subd_nurbs_packs_probe.py',
        'join_probe.py','merge_edges_probe.py','client.py','rhino_worker.py','structural_sdk.py'],
        'VibocerosOracleSubDNurbsPacks20261105',timeout=1800)
