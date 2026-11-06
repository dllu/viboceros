"""Capture public-zero-finder trials beside native tangent queries."""
from .capture_family import capture
from .tangent_zero_sdk import request
from .structural_sdk import PROBES
if __name__=='__main__':
 module,deps=PROBES['tangent_zero_sdk']
 capture('tangent_zero_sdk',request(),['generate_tangent_zero_sdk.py','structural_sdk.py','rhino_worker.py','client.py',*(n+'.py'for n in(module,*deps))],'VibocerosOracleTangentZeroSdk20261106',timeout=1200)
