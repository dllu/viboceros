"""Capture native application preference changes only under private Xvfb."""
from .capture_family import capture
from .use_extrusions_cases import cases
from .structural_sdk import PROBES

if __name__=='__main__':
    modules=['use_extrusions_probe',*PROBES['use_extrusions'][1]]
    capture('use_extrusions',dict(protocol_version=1,iterations=1,
            tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=cases()),
            ['generate_use_extrusions.py',*[name+'.py' for name in modules],'structural_sdk.py','rhino_worker.py','client.py'],
            'VibocerosOracleUseExtrusions20261106',timeout=900)
