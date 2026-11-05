"""Fixed option keystrokes delivered only to the owned private Rhino window."""
import json
import os
import re
import subprocess
from .group_picking import IdlePicker


class EdgeZoomPicker(IdlePicker):
    def send_input(self, name, x, y, window):
        match = re.fullmatch(r'@edge-zoom:[0-9a-f]{32}:\d{1,2}:(All|Current|Next|Previous|Mark|Enter|Escape)', name)
        if match is None:
            raise ValueError('invalid owned edge Zoom input marker')
        token = match[1]
        if token not in ('Enter', 'Escape'):
            subprocess.run(['xdotool', 'windowactivate', '--sync', window, 'type', '--clearmodifiers', '_'+token], check=True, timeout=10)
        key = 'Escape' if token == 'Escape' else 'Return'
        subprocess.run(['xdotool', 'windowactivate', '--sync', window, 'key', '--clearmodifiers', key], check=True, timeout=10)
        return True

    def __call__(self, job, owned_pids):
        super().__call__(job, owned_pids)
        accepted = sorted(name for name in self.seen if name.startswith('@edge-zoom:'))
        temporary = job / 'edge-zoom-ack.json.tmp'
        temporary.write_text(json.dumps(accepted), encoding='utf-8')
        os.replace(temporary, job / 'edge-zoom-ack.json')
