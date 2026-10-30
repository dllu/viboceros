"""Independent center, frame and source-tag holdouts for SubD Cap."""
from .capture_family import capture
from .generate_subd_cap_regions import request as base
from .subd_cap_regions_cases import HOLDOUT_CASES
from .mesh_insert_inputs import save_sources


def request():
    query=base();query['operations']=[dict(op='subd_cap_regions',id=name,case=name)for name in HOLDOUT_CASES];return query


if __name__=='__main__':
    capture('subd_cap_holdouts',request(),['generate_subd_cap_holdouts.py','generate_subd_cap_regions.py',
        'subd_cap_regions_cases.py','subd_cap_regions_probe.py','mesh_insert_inputs.py','crease_probe.py',
        'crease_cases.py','subdivide_probe.py','subd_box_probe.py','join_probe.py','merge_edges_probe.py',
        'rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleSubDCapHoldouts20261029',timeout=900)
    save_sources('subd_cap_holdouts')
