"""Capture shared extrusion conversions through native commands under Xvfb."""
from .capture_family import capture
from .convert_extrusion_cases import ASSETS,cases
from .structural_sdk import PROBES

if __name__=='__main__':
    modules=['convert_extrusion_probe',*PROBES['convert_extrusion'][1]]
    capture('convert_extrusion',dict(protocol_version=1,iterations=1,
            tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=cases()),
            ['generate_convert_extrusion.py',*[name+'.py' for name in modules],
             'structural_sdk.py','rhino_worker.py','client.py',*['fixtures/extrusions/'+name+'.3dm' for name in ASSETS]],
            'VibocerosOracleConvertExtrusion20261105',timeout=1200)
