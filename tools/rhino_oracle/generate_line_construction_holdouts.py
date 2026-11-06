"""Capture fresh complete line prompts and independent spatial inputs."""
from .capture_family import capture
from .line_construction_cases import request
from .line_construction_holdouts import HOLDOUTS
from .structural_sdk import PROBES
if __name__=='__main__':
    module,deps=PROBES['line_construction']
    capture('line_construction_holdouts',request(list(HOLDOUTS)),['generate_line_construction_holdouts.py','structural_sdk.py','rhino_worker.py','client.py',
        *(n+'.py' for n in (module,*deps))],'VibocerosOracleLineHoldouts20261106',timeout=1200)
