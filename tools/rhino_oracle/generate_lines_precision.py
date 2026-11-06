"""Capture fixed Lines coincidence and completion boundaries."""
from .capture_family import capture
from .lines_cases import request
from .lines_precision_cases import PRECISION
from .structural_sdk import PROBES
if __name__=='__main__':
    module,deps=PROBES['lines_command']
    capture('lines_precision',request(list(PRECISION)),['generate_lines_precision.py','structural_sdk.py','rhino_worker.py','client.py',
        *(n+'.py' for n in (module,*deps))],'VibocerosOracleLinesPrecision20261106',timeout=1200)
