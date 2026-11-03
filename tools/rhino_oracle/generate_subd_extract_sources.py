"""Freeze editable source archives independently of command results."""
from .capture_family import capture
from .subd_extract_cases import all_cases
query=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-8,relative=1e-12,angular=1e-8),
           operations=[dict(op='subd_extract_sources',id=scene,scene=scene) for scene in all_cases()])
if __name__=='__main__':
    capture('subd_extract_sources',query,['generate_subd_extract_sources.py','subd_extract_sources.py',
        'subd_extract_cases.py','subd_join_probe.py','subd_join_cases.py','subd_box_probe.py',
        'client.py','rhino_worker.py','structural_sdk.py','merge_edges_probe.py'],'VibocerosOracleSubDExtractSources20261102',timeout=900)
