"""Capture Lines cancellation/completion through the public command-file runner."""
from .capture_family import capture
from .lines_cases import request
from .lines_macro_cases import MACROS
from .structural_sdk import PROBES
if __name__=='__main__':
    module,deps=PROBES['lines_command']
    capture('lines_macros',request(list(MACROS)),['generate_lines_macros.py','structural_sdk.py','rhino_worker.py','client.py',
        *(n+'.py' for n in (module,*deps))],'VibocerosOracleLinesMacros20261106',timeout=1200)
