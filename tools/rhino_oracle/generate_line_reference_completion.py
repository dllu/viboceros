"""Capture independent Line reference and pending-redo controls."""
from .capture_family import capture
from .line_reference_cases import request
from .line_reference_completion import COMPLETION
from .structural_sdk import PROBES
if __name__=='__main__':
 module,deps=PROBES['line_reference']
 capture('line_reference_completion',request(list(COMPLETION)),['generate_line_reference_completion.py','structural_sdk.py','rhino_worker.py','client.py','catenary_point_picker.py','group_picking.py',*(n+'.py'for n in(module,*deps))],'VibocerosOracleLineRefCompletion20261106',timeout=1200)
