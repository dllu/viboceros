"""Capture type selection only in a dedicated private Xvfb Rhino session."""
from .capture_family import capture
from .type_selection_cases import cases

if __name__=='__main__':
    capture('type_selection',dict(protocol_version=1,iterations=1,
            tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=cases()),
            ['generate_type_selection.py','type_selection_cases.py','type_selection_scene.py','type_selection_probe.py',
             'join_probe.py','merge_edges_probe.py','structural_sdk.py','rhino_worker.py','client.py'],
            'VibocerosOracleTypeSelection20261105',timeout=1200)
