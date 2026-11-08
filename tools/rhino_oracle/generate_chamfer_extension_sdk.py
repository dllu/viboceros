"""Capture public extension API controls on a private Xvfb display."""
from .capture_family import capture
from .chamfer_extension_sdk import request
if __name__=='__main__':
    capture('chamfer_extension_sdk',request(),[
        'generate_chamfer_extension_sdk.py','chamfer_extension_sdk.py','structural_sdk.py',
        'rhino_worker.py','client.py',
    ],'VibocerosOracleChamferExtensionSDK20261108',timeout=1200)
