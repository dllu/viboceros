"""Capture public Fillet contact selection controls on private Xvfb."""
from .capture_family import capture
from .fillet_continuation_sdk import request
if __name__=='__main__':
    capture('fillet_continuation_sdk',request(),[
        'generate_fillet_continuation_sdk.py','fillet_continuation_sdk.py','structural_sdk.py','rhino_worker.py','client.py',
        *['observations/fillet_continuation_'+name+'.json.gz'for name in ['pilot','holdouts','verification']],
    ],'VibocerosOracleFilletContinuationSDK20261108',timeout=1500)
