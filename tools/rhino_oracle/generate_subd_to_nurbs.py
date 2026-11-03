"""Freeze SubD to NURBS packing, continuity and limit-edge controls."""
from .capture_family import capture
from .subd_to_nurbs_probe import cases

if __name__=='__main__':
    query=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=cases())
    capture('subd_to_nurbs_sdk',query,['generate_subd_to_nurbs.py','subd_to_nurbs_probe.py',
            'subd_join_probe.py','subd_join_cases.py','subd_box_probe.py','client.py',
            'rhino_worker.py','structural_sdk.py','merge_edges_probe.py'],
            'VibocerosOracleSubDToNurbs20261104',timeout=1800)
