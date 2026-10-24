"""Let the owned getter process pointer motion before accepting a region pick."""
import re
import subprocess
import time
from .component_picking import ComponentPicker


class MeshTrimPicker(ComponentPicker):
    def send_input(self, name, x, y, window):
        if name.startswith('@component-finish:'):
            time.sleep(1.)
        if name.startswith('@component-click:'):
            if re.fullmatch(r'@component-click:[A-Za-z0-9_.-]{1,100}:\d{1,2}:plain', name) is None:
                raise ValueError('invalid owned MeshTrim region pick')
            time.sleep(1.)
            subprocess.run(['xdotool', 'windowactivate', '--sync', window,
                            'mousemove', str(x), str(y)], check=True, timeout=10)
            time.sleep(.5)
        return super().send_input(name, x, y, window)
