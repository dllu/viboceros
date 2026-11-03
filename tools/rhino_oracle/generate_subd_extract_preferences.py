"""Capture corner option lifetime across independent extraction commands."""
from .capture_family import capture
query=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-8,relative=1e-12,angular=1e-8),
           operations=[dict(op='subd_extract_preferences',id='corner_option_lifetime')])
if __name__=='__main__':
    capture('subd_extract_preferences',query,['generate_subd_extract_preferences.py','subd_extract_preferences.py',
        'subd_extract_probe.py','subd_extract_cases.py','subd_join_probe.py','subd_join_cases.py','subd_box_probe.py',
        'client.py','rhino_worker.py','structural_sdk.py','join_probe.py','merge_edges_probe.py'],
        'VibocerosOracleSubDExtractPreferences20261102',timeout=900)
