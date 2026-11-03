"""Freeze multiple-source OffsetSubD object order and grouping."""
from .capture_family import capture
from .subd_offset_batch import cases
query=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-8,relative=1e-12,angular=1e-8),operations=cases())
if __name__=='__main__':
    capture('subd_offset_batch',query,['generate_subd_offset_batch.py','subd_offset_batch.py','subd_offset_command_probe.py',
        'subd_offset_command_cases.py','subd_offset_cases.py','subd_extract_cases.py','subd_join_cases.py','subd_join_probe.py','subd_box_probe.py',
        'join_probe.py','merge_edges_probe.py','client.py','rhino_worker.py','structural_sdk.py'],
        'VibocerosOracleSubDOffsetBatch20261102',timeout=900)
