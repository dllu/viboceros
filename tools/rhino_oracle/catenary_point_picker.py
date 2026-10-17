"""Click only after a public mouse callback acknowledges the owned point target."""
import json
import subprocess
import time

from .group_picking import IdlePicker
from .client import OracleProtocolError


class CatenaryPointPicker(IdlePicker):
    def __init__(self, request):
        super().__init__()
        self.cases = {
            '@catenary-click:' + op['id'] + '-' + str(i): op['id'] + '-' + str(i)
            for op in request['operations']
            for i, action in enumerate(op['actions'])
            if isinstance(action, dict) and 'click' in action
        }
        self.moved = {}

    def __call__(self, job, pids):
        self.job = job
        super().__call__(job, pids)

    def send_input(self, name, x, y, window):
        if name not in self.cases:
            raise OracleProtocolError('foreign Catenary click marker')
        ready = self.job / ('catenary-click-ready-' + self.cases[name] + '.json')
        if not ready.exists():
            if name not in self.moved or time.monotonic() - self.moved[name] >= 1.:
                subprocess.run([
                    'xdotool', 'windowactivate', '--sync', window,
                    'mousemove', str(int(x) + 1), y, 'mousemove', x, y,
                ], check=True, timeout=10)
                self.moved[name] = time.monotonic()
            return False
        if json.loads(ready.read_text()) != name:
            raise OracleProtocolError('foreign Catenary motion acknowledgement')
        subprocess.run([
            'xdotool', 'windowactivate', '--sync', window, 'click', '1',
        ], check=True, timeout=10)
        return True

    def record_diagnostics(self, response):
        if self.seen != set(self.cases):
            raise OracleProtocolError('incomplete owned Catenary point clicks')
