"""Select an owned mesh face explicitly with Rhino's component modifiers."""
import subprocess
from .group_picking import IdlePicker


class FaceCollapsePicker(IdlePicker):
    def send_input(self, name, x, y, window):
        if not name.startswith('@face-collapse:'):
            raise ValueError('unexpected face collapse input marker')
        subprocess.run(['xdotool','windowactivate','--sync',window,
                        'keydown','ctrl','keydown','shift','mousemove',x,y,
                        'click','1','keyup','shift','keyup','ctrl',
                        'key','--clearmodifiers','Return'],check=True,timeout=10)
        return True
