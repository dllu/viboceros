"""Capture independently authored SubD openings and all Cap option combinations."""
from .capture_family import capture
from .subd_cap_regions_cases import BASIC_CASES
from .mesh_insert_inputs import save_sources


def request():
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-9,relative=1e-12,angular=1e-10),
        operations=[dict(op='subd_cap_regions',id=name,case=name)for name in BASIC_CASES])


if __name__=='__main__':
    capture('subd_cap_regions',request(),['generate_subd_cap_regions.py','subd_cap_regions_cases.py',
        'subd_cap_regions_probe.py','mesh_insert_inputs.py','crease_probe.py','crease_cases.py',
        'subdivide_probe.py','subd_box_probe.py','join_probe.py','merge_edges_probe.py',
        'rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleSubDCapRegions20261029',timeout=900)
    save_sources('subd_cap_regions')
