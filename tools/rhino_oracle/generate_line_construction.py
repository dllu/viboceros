"""Capture fixed basic, symmetric, vertical, directed and angled lines."""
from .capture_family import capture
from .line_construction_cases import request
from .structural_sdk import PROBES
if __name__=='__main__':
    module,deps=PROBES['line_construction']
    capture('line_construction',request(),['generate_line_construction.py','structural_sdk.py','rhino_worker.py','client.py',
        *(n+'.py' for n in (module,*deps))],'VibocerosOracleLineConstruction20261106',timeout=1200)
