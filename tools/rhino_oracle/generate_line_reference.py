"""Capture independent curve-end Line Extension constructions."""
from .capture_family import capture
from .line_reference_cases import request
from .structural_sdk import PROBES
if __name__=='__main__':
 module,deps=PROBES['line_reference']
 capture('line_reference',request(),['generate_line_reference.py','structural_sdk.py','rhino_worker.py','client.py','catenary_point_picker.py','group_picking.py',*(n+'.py'for n in(module,*deps))],'VibocerosOracleLineReference20261106',timeout=1200)
