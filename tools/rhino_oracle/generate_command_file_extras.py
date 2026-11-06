"""Capture prescribed files with the public ReadCommandFile command."""
from .capture_family import capture
from .command_file_cases import request,EXTRAS
from .structural_sdk import PROBES
if __name__=='__main__':
    module,deps=PROBES['command_file']
    capture('command_file_extras',request(list(EXTRAS)),['generate_command_file_extras.py','structural_sdk.py','rhino_worker.py','client.py',
        *(n+'.py' for n in (module,*deps))],'VibocerosOracleCommandFileExtras20261106',timeout=1200)
