"""Documented Roadlike defaults use Rhino's public plane query before sweeping."""
from .capture_family import capture
from .generate_sweep1_roadlike_default import request


if __name__ == '__main__':
    capture('sweep1_roadlike_automatic',request(command=False),[
        'generate_sweep1_roadlike_automatic.py','generate_sweep1_roadlike_default.py',
        'rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleRoadlikeAutomatic20261027',timeout=180)
