"""Capture prescribed adjoining-line getter workflows in private Rhino."""
from .capture_family import capture
from .lines_cases import request
from .structural_sdk import PROBES
if __name__=='__main__':
    module,deps=PROBES['lines_command']
    capture('lines_command',request(),['generate_lines.py','structural_sdk.py','rhino_worker.py','client.py',
        *(n+'.py' for n in (module,*deps))],'VibocerosOracleLines20261106',timeout=1200)
