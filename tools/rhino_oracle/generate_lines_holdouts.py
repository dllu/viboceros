"""Capture independent reversed, short, spatial and history Lines inputs."""
from .capture_family import capture
from .lines_cases import request,HOLDOUTS
from .structural_sdk import PROBES
if __name__=='__main__':
    module,deps=PROBES['lines_command']
    capture('lines_holdouts',request(HOLDOUTS),['generate_lines_holdouts.py','structural_sdk.py','rhino_worker.py','client.py',
        *(n+'.py' for n in (module,*deps))],'VibocerosOracleLinesHoldouts20261106',timeout=1200)
