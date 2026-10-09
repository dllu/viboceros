from pathlib import Path
import json, shutil
from unittest.mock import patch
from tools.rhino_oracle.client import OracleClient

original_copy = shutil.copyfile
helper = Path('/tmp/viboceros-edge-mark-helper.py')

def scoped_copy(source, destination, *args, **kwargs):
    if Path(destination).name == 'edge_analysis_probe.py':
        return original_copy(helper, destination, *args, **kwargs)
    return original_copy(source, destination, *args, **kwargs)

request = json.loads(Path('/tmp/viboceros-edge-mark-request.json').read_text())
with patch('shutil.copyfile', scoped_copy):
    response = OracleClient(settings_scheme='VibocerosOracleEdgeMark20261009').run_rhino(request, timeout=240)
Path('/tmp/viboceros-edge-mark-native.json').write_text(json.dumps(response, indent=2) + '\n')
