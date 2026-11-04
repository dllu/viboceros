"""Native affine and subdivided variable-crease holdouts on private Xvfb."""
from .capture_family import capture
from .subd_soft_nurbs_cases import variable_cases,ASSETS

if __name__=='__main__':
    query=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=variable_cases())
    capture('subd_soft_nurbs_variable',query,['generate_subd_soft_nurbs_variable.py','subd_soft_nurbs_cases.py','subd_soft_nurbs_probe.py',
        'subd_packing_cases.py','subd_g1x_probe.py','subd_continuity_probe.py','subd_g0_probe.py','subd_to_nurbs_probe.py',
        'subd_join_probe.py','subd_join_cases.py','subd_box_probe.py','join_probe.py','client.py','rhino_worker.py',
        'structural_sdk.py','merge_edges_probe.py','fixtures/subd_soft_nurbs_inputs/provenance.json',
        *['fixtures/subd_soft_nurbs_inputs/'+name+'.3dm' for name in ASSETS]],
        'VibocerosOracleSubDSoftNurbsVariable20261104',timeout=2400)
