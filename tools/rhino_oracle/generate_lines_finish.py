"""Capture complete Lines return/Enter controls and fixed admission boundaries."""
from .capture_family import capture
from .lines_cases import request
from .lines_finish_cases import FINISH
from .structural_sdk import PROBES
if __name__=='__main__':
    module,deps=PROBES['lines_command']
    capture('lines_finish',request(list(FINISH)),['generate_lines_finish.py','structural_sdk.py','rhino_worker.py','client.py',
        *(n+'.py' for n in (module,*deps))],'VibocerosOracleLinesFinish20261106',timeout=1200)
