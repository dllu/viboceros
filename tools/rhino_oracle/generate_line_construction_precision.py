"""Capture independent Line precision and command-file controls."""
from .capture_family import capture
from .line_construction_cases import request
from .line_construction_precision import PRECISION
from .structural_sdk import PROBES
if __name__=='__main__':
    module,deps=PROBES['line_construction']
    capture('line_construction_precision',request(list(PRECISION)),['generate_line_construction_precision.py','structural_sdk.py','rhino_worker.py','client.py',
        *(n+'.py' for n in (module,*deps))],'VibocerosOracleLinePrecision20261106',timeout=1200)
