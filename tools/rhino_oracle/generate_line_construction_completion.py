"""Capture independent Line completion and command-file controls."""
from .capture_family import capture
from .line_construction_cases import request
from .line_construction_completion import COMPLETION
from .structural_sdk import PROBES
if __name__=='__main__':
    module,deps=PROBES['line_construction']
    capture('line_construction_completion',request(list(COMPLETION)),['generate_line_construction_completion.py','structural_sdk.py','rhino_worker.py','client.py',
        *(n+'.py' for n in (module,*deps))],'VibocerosOracleLineCompletion20261106',timeout=1200)
