"""Deliver Chamfer clicks and Escape only to the verified private Rhino window."""
import subprocess
from .arc_point_picker import ArcPointPicker


class ChamferPointPicker(ArcPointPicker):
    key_prefix='@chamfer-escape:'
    def __init__(self, request):
        super().__init__(request)
        self.keys = {self.key_prefix + op['id'] + '-' + str(i): (op['id'], i)
                     for op in request['operations'] for i, a in enumerate(op['actions']) if a == 'Cancel'}
        self.cases.update(self.keys)

    def send_input(self, name, x, y, window):
        if name in self.keys:
            subprocess.run(['xdotool', 'windowactivate', '--sync', window, 'key', '--clearmodifiers', 'Escape'], check=True, timeout=10)
            return True
        return super().send_input(name, x, y, window)

    def record_diagnostics(self, response):
        super().record_diagnostics(response)
        for row in response['results']:
            row['value']['external_keys'] = [dict(action=i, key='Escape', private_owned_window=True)
                for marker, (name, i) in self.keys.items() if name == row['id'] and marker in self.seen]
