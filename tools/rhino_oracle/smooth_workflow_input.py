"""One real Escape per bounded Smooth getter in the owned private window."""
import re
import subprocess
import time

from .client import OracleProtocolError, _read_optional_text, _rhino_window_for_pids
from .smooth_workflow_probe import validate_request


class SmoothKeyboard:
    def __init__(self, request):
        validate_request(request)
        self.ids = [op['id'] for op in request['operations']]
        self.cases = {op['id']: op['case'] for op in request['operations']
                      if op['case'].startswith('keyboard_')}
        self.ready = {}
        self.sent = set()

    def __call__(self, job, owned_pids):
        progress = _read_optional_text(job / 'worker-progress.log')
        for name in re.findall(r'^SMOOTH_ESCAPE ([A-Za-z0-9_-]{1,80})$', progress, re.MULTILINE):
            if name not in self.cases:
                raise OracleProtocolError('unexpected Smooth keyboard marker')
            if name in self.sent:
                continue
            # BeginCommand precedes the getter; allow the fixed macro prefix
            # to reach its sole _Pause before sending input.
            ready = self.ready.setdefault(name, time.monotonic() + 1.)
            if not owned_pids or time.monotonic() < ready:
                continue
            window = _rhino_window_for_pids(owned_pids)
            if window is None:
                continue
            subprocess.run(['xdotool', 'windowactivate', '--sync', window,
                            'key', '--clearmodifiers', 'Escape'], check=True, timeout=10)
            self.sent.add(name)

    def record_diagnostics(self, response):
        if [row['id'] for row in response['results']] != self.ids or self.sent != set(self.cases):
            raise OracleProtocolError('incomplete Smooth keyboard evidence')
        for row in response['results']:
            if row['id'] in self.sent:
                row['value']['host_input'] = dict(key='Escape', private_owned_window=True)
