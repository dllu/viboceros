"""Capture staged point-grid commands in a private owned Rhino document."""
from .capture_family import capture
from .point_grid_input_cases import request
from .structural_sdk import PROBES
if __name__=='__main__':
    module,deps=PROBES['point_grid_input']
    capture('point_grid_input',request(),['generate_point_grid_input.py','structural_sdk.py','rhino_worker.py','client.py',
        *(n+'.py' for n in (module,*deps))],'VibocerosOraclePointGridInput20261106',timeout=1200)
