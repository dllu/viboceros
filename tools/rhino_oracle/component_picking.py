"""Sequenced component inputs confined to an owned private Rhino window."""
import re
import subprocess
import time
from contextlib import ExitStack

from .hole_picking import HolePicker


class ComponentPicker(HolePicker):
    def send_input(self, name, x, y, window):
        if name.startswith('@component-finish:'):
            match = re.fullmatch(r'@component-finish:[A-Za-z0-9_.-]{1,100}:(Enter|Cancel)', name)
            if match is None: raise ValueError('invalid owned component finish')
            key = 'Return' if match[1] == 'Enter' else 'Escape'
            subprocess.run(['xdotool', 'windowactivate', '--sync', window], check=True, timeout=10)
            try:
                subprocess.run(['xdotool', 'keydown', key], check=True, timeout=10)
                time.sleep(.15)
            finally:
                subprocess.run(['xdotool', 'keyup', key], check=True, timeout=10)
            return True
        if not name.startswith('@component-'):
            return super().send_input(name, x, y, window)
        if name.startswith('@component-key:'):
            match = re.fullmatch(r'@component-key:[A-Za-z0-9_.-]{1,100}:\d{1,2}:(None|Undo|Copy=Yes|Copy=No|OutputLayer=Current|OutputLayer=Input)', name)
            if match is None: raise ValueError('invalid owned component key')
            subprocess.run(['xdotool', 'windowactivate', '--sync', window, 'type', '--clearmodifiers', match[1]], check=True, timeout=10)
            subprocess.run(['xdotool', 'windowactivate', '--sync', window, 'key', '--clearmodifiers', 'Return'], check=True, timeout=10)
            return True
        match = re.fullmatch(r'@component-(click|window):[A-Za-z0-9_.-]{1,100}:\d{1,2}:(plain|ctrl|shift|sub|alt)(?::(\d{1,5}):(\d{1,5}))?', name)
        if match is None or (match[1] == 'window') != (match[3] is not None):
            raise ValueError('invalid owned component input')
        keys = dict(plain=[], ctrl=['ctrl'], shift=['shift'], sub=['ctrl', 'shift'], alt=['alt'])[match[2]]
        start = [int(x), int(y)]
        end = [int(match[3]), int(match[4])] if match[1] == 'window' else start
        with ExitStack() as cleanup:
            subprocess.run(['xdotool', 'windowactivate', '--sync', window], check=True, timeout=10)
            for key in keys:
                cleanup.callback(subprocess.run, ['xdotool', 'keyup', key], check=True, timeout=10)
                subprocess.run(['xdotool', 'keydown', key], check=True, timeout=10)
            cleanup.callback(subprocess.run, ['xdotool', 'mouseup', '1'], check=True, timeout=10)
            subprocess.run(['xdotool', 'mousemove', *map(str, start), 'mousedown', '1'], check=True, timeout=10)
            if match[1] == 'window':
                for step in range(1, 11):
                    point = [start[axis] + (end[axis] - start[axis]) * step // 10 for axis in range(2)]
                    subprocess.run(['xdotool', 'mousemove', *map(str, point)], check=True, timeout=10)
                    time.sleep(.04)
        return True
