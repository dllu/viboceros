"""Capture complete Plane getter recipes without inherited display context."""
from .capture_family import capture
from .plane_surface_cases import request, COMPLETION_CASES
from .structural_sdk import PROBES

if __name__ == '__main__':
    module, dependencies = PROBES['plane_surface_command']
    capture('plane_completion', request(list(COMPLETION_CASES)),
            ['generate_plane_completion.py',*[name+'.py' for name in (module,*dependencies)],
             'structural_sdk.py','rhino_worker.py','client.py'],
            'VibocerosOraclePlaneCompletion20261106', timeout=1000)
