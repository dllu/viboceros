"""Capture public ConvertExtrusion options on a private native display."""
from .capture_family import capture
from .convert_extrusion_cases import ASSETS,pilot_cases
from .structural_sdk import PROBES

if __name__=='__main__':
    modules=['convert_extrusion_probe',*PROBES['convert_extrusion'][1]]
    capture('convert_extrusion_pilot',dict(protocol_version=1,iterations=1,
            tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=pilot_cases()),
            ['generate_convert_extrusion_pilot.py',*[name+'.py' for name in modules],
             'structural_sdk.py','rhino_worker.py','client.py',*['fixtures/extrusions/'+name+'.3dm' for name in ASSETS]],
            'VibocerosOracleConvertExtrusionPilotClean2_20261105',timeout=900)
