"""Freeze actual extrusion and block-selection command observations."""
from .capture_family import capture
from .typed_selection_probe import request

if __name__ == '__main__':
    capture('typed_selection', request(), [
        'generate_typed_selection.py', 'typed_selection_probe.py', 'join_probe.py', 'merge_edges_probe.py',
        'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleTypedSelectionIdle20261029', timeout=600)
