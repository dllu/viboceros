"""Check whether shared point getters use whole-expression length suffixes."""
from .capture_family import capture
from .line_construction_cases import request,QUANTITY_INPUT
from .structural_sdk import PROBES

if __name__=='__main__':
    module,deps=PROBES['line_construction']
    capture('arc_blend_point_quantities',request(list(QUANTITY_INPUT)),[
        'generate_arc_blend_point_quantities.py','structural_sdk.py','rhino_worker.py',
        'client.py',*(name+'.py'for name in(module,*deps)),
    ],'VibocerosOracleArcBlendPointQuantities20261106',timeout=900)
