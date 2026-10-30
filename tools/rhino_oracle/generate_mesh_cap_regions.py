"""Capture Cap on independently authored double-precision wall boundaries."""
from .capture_family import capture
from .mesh_cap_regions_cases import CASES,REGIONS


def request():
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-9,relative=1e-12,angular=1e-15),
                operations=[dict(op='mesh_cap_regions',id=name,case=name)for name in REGIONS])


if __name__=='__main__':
    capture('mesh_cap_regions',request(),['generate_mesh_cap_regions.py','mesh_cap_regions_cases.py',
        'mesh_cap_regions_probe.py','crease_probe.py','crease_cases.py','subdivide_probe.py',
        'subd_box_probe.py','join_probe.py','merge_edges_probe.py','rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleMeshCapRegions20261029',timeout=600)
