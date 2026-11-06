"""Capture independent Lines admission boundaries and two-segment return."""
from .capture_family import capture
from .lines_cases import request
from .lines_boundary_cases import BOUNDARIES
from .structural_sdk import PROBES
if __name__=='__main__':
    module,deps=PROBES['lines_command']
    capture('lines_boundaries',request(list(BOUNDARIES)),['generate_lines_boundaries.py','structural_sdk.py','rhino_worker.py','client.py',
        *(n+'.py' for n in (module,*deps))],'VibocerosOracleLinesBoundaries20261106',timeout=1200)
