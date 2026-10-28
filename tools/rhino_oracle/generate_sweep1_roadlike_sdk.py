"""RhinoCommon's Roadlike frame with an Unset normal is captured independently."""
from .capture_family import capture
from .generate_sweep1_roadlike_default import request


if __name__ == '__main__':
    capture('sweep1_roadlike_sdk_default',request(command=False),[
        'generate_sweep1_roadlike_sdk.py','generate_sweep1_roadlike_default.py',
        'rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleRoadlikeSDK20261027',timeout=180)
