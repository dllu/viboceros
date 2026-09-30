"""Mouse navigation confined to the oracle's owned Rhino window and Xvfb."""

import subprocess
import time

from .group_picking import IdlePicker


class CameraNavigator(IdlePicker):
    def __init__(self, request):
        super().__init__()
        self.drags = {operation["id"]: operation["mouse_drag"]
                      for operation in request["operations"] if "mouse_drag" in operation}

    def send_input(self, name, x, y, window):
        if name not in self.drags:
            return super().send_input(name, x, y, window)
        dx, dy = self.drags[name]
        x, y = int(x), int(y)
        time.sleep(1.0)
        try:
            subprocess.run(["xdotool", "windowactivate", "--sync", window,
                            "mousemove", str(x), str(y), "mousedown", "3"],
                           check=True, timeout=10)
            for step in range(1, 11):
                subprocess.run(["xdotool", "mousemove", str(x + dx * step // 10),
                                str(y + dy * step // 10)], check=True, timeout=10)
                time.sleep(0.04)
        finally:
            subprocess.run(["xdotool", "mouseup", "3"], check=True, timeout=10)
        time.sleep(0.5)
        # A horizontal camera can look parallel to the CPlane, so a click may
        # have no point to accept. Cancel the disposable EvaluatePt prompt.
        subprocess.run(["xdotool", "key", "--clearmodifiers", "Escape"],
                       check=True, timeout=10)
        return True
