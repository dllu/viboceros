"""Capture selection retention when type filters have no eligible matches."""
from .capture_family import capture
from .type_selection_cases import empty_cases
from .structural_sdk import PROBES

if __name__=='__main__':
    modules=['type_selection_probe',*PROBES['type_selection'][1]]
    capture('type_selection_empty',dict(protocol_version=1,iterations=1,
            tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=empty_cases()),
            ['generate_type_selection_empty.py',*[name+'.py' for name in modules],
             'structural_sdk.py','rhino_worker.py','client.py'],
            'VibocerosOracleTypeSelectionEmpty20261105',timeout=1200)
