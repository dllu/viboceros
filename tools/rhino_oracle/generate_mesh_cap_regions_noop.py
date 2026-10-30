"""Already closed inputs preserve a separate Native Undo/Redo witness."""
from .capture_family import capture
from .generate_mesh_cap_regions import request as base
from .mesh_cap_regions_cases import NOOP_CASES


def request():
    q=base();q['operations']=[dict(op='mesh_cap_regions',id=name,case=name)for name in NOOP_CASES];return q


if __name__=='__main__':
    capture('mesh_cap_regions_noop',request(),['generate_mesh_cap_regions_noop.py',
        'generate_mesh_cap_regions.py','mesh_cap_regions_cases.py','mesh_cap_regions_probe.py',
        'crease_probe.py','crease_cases.py','subdivide_probe.py','subd_box_probe.py',
        'join_probe.py','merge_edges_probe.py','rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleMeshCapRegionNoop20261029',timeout=600)
