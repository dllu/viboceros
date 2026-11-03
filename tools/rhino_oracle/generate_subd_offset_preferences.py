"""Freeze native OffsetSubD defaults and option clocks."""
from .capture_family import capture
query=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-8,relative=1e-12,angular=1e-8),
           operations=[dict(op='subd_offset_preferences',id='option_lifetime')])
if __name__=='__main__':
    capture('subd_offset_preferences',query,['generate_subd_offset_preferences.py','subd_offset_preferences.py','subd_offset_command_probe.py',
        'subd_offset_command_cases.py','subd_offset_cases.py','subd_extract_cases.py','subd_join_cases.py','subd_join_probe.py','subd_box_probe.py',
        'join_probe.py','merge_edges_probe.py','client.py','rhino_worker.py','structural_sdk.py'],
        'VibocerosOracleSubDOffsetPreferences20261102',timeout=900)
