"""Hole command rectangles confined to the oracle's owned private viewport."""
import re
import subprocess
import time

from .group_picking import IdlePicker


class HolePicker(IdlePicker):
    def send_input(self, name, x, y, window):
        if name.startswith("@hole-finish:"):
            match = re.fullmatch(r"@hole-finish:[A-Za-z0-9_.-]{1,100}:(Enter|Cancel)", name)
            if match is None: raise ValueError("invalid owned hole finish")
            subprocess.run(["xdotool", "windowactivate", "--sync", window, "key", "--clearmodifiers",
                            "Return" if match.group(1) == "Enter" else "Escape"], check=True, timeout=10)
            return True
        if not name.startswith("@hole-window:"):
            return super().send_input(name, x, y, window)
        match = re.fullmatch(r"@hole-window:[A-Za-z0-9_.-]{1,100}:(sub|plain):(\d{1,5}):(\d{1,5})", name)
        if match is None:
            raise ValueError("invalid owned hole rectangle")
        start, end = (int(x), int(y)), tuple(map(int, match.groups()[1:]))
        subobjects = match.group(1) == "sub"
        try:
            if subobjects:
                subprocess.run(["xdotool", "keydown", "ctrl", "keydown", "shift"], check=True, timeout=10)
            subprocess.run(["xdotool", "windowactivate", "--sync", window,
                            "mousemove", str(start[0]), str(start[1]), "mousedown", "1"], check=True, timeout=10)
            for step in range(1, 11):
                point = [start[axis] + (end[axis] - start[axis]) * step // 10 for axis in range(2)]
                subprocess.run(["xdotool", "mousemove", str(point[0]), str(point[1])], check=True, timeout=10)
                time.sleep(.04)
        finally:
            subprocess.run(["xdotool", "mouseup", "1"], check=True, timeout=10)
            if subobjects:
                subprocess.run(["xdotool", "keyup", "shift", "keyup", "ctrl"], check=True, timeout=10)
        return True
