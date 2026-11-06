"""Capture independent Line reference and pending-redo controls."""
from .capture_family import capture
from .line_reference_cases import request
from .line_reference_holdouts import HOLDOUTS
from .structural_sdk import PROBES
if __name__=='__main__':
 module,deps=PROBES['line_reference']
 capture('line_reference_holdouts',request(list(HOLDOUTS)),['generate_line_reference_holdouts.py','structural_sdk.py','rhino_worker.py','client.py','catenary_point_picker.py','group_picking.py',*(n+'.py'for n in(module,*deps))],'VibocerosOracleLineRefHoldouts20261106',timeout=1200)
