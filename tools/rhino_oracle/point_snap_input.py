"""Owned, nonblocking motion/settling controls for point-snap diagnostics."""
import subprocess
import time
from .client import OracleProtocolError
from .group_picking import IdlePicker
from .point_snap_probe import validate_request


class PointSnapPicker(IdlePicker):
    def __init__(self, request):
        validate_request(request)
        super().__init__()
        self.delays = {"@point:"+op["id"]:op.get("input_settle_ms",0) for op in request["operations"]}
        self.detours = {"@point:"+op["id"]:op.get("input_detour",[1,0]) for op in request["operations"]}
        self.explicit_detours = {"@point:"+op["id"] for op in request["operations"] if "input_detour" in op}
        self.moved = {}
        self.events = {}

    def send_input(self, name, x, y, window):
        if name not in self.delays:
            raise OracleProtocolError("unexpected owned point input marker: " + name)
        delay = self.delays[name]
        if not delay:
            return super().send_input(name,x,y,window)
        if name not in self.moved:
            # Point-snap calibration reserves one pixel inside the viewport on
            # every side. A one-pixel detour forces motion even for repeat clicks.
            dx,dy = self.detours[name]
            subprocess.run(["xdotool","windowactivate","--sync",window,
                            "mousemove",str(int(x)+dx),str(int(y)+dy),"mousemove",x,y],check=True,timeout=10)
            self.moved[name] = (window,x,y,time.monotonic())
            return False
        previous,px,py,moved_at = self.moved[name]
        if (window,x,y) != (previous,px,py):
            raise OracleProtocolError("owned point window or target changed after motion")
        elapsed = time.monotonic()-moved_at
        if elapsed*1000 < delay: return False
        subprocess.run(["xdotool","windowactivate","--sync",window,"click","1"],check=True,timeout=10)
        self.events[name] = dict(requested_settle_ms=delay,motion_to_click_ms=elapsed*1000,detour_pixels=1)
        if name in self.explicit_detours:
            self.events[name]["detour"] = list(self.detours[name])
        return True

    def record_diagnostics(self, response):
        """Append host input evidence; never replace the Rhino result fields."""
        if ["@point:"+row["id"] for row in response["results"]] != list(self.delays):
            raise OracleProtocolError("owned point response ids do not match requested order")
        pending = []
        for row in response["results"]:
            name = "@point:"+row["id"]
            if not isinstance(row["value"],dict) or "input_motion" in row["value"]:
                raise OracleProtocolError("point response has invalid or conflicting host input evidence")
            if not self.delays[name]: continue
            if name not in self.seen or name not in self.events:
                raise OracleProtocolError("point response lacks completed owned input")
            pending.append((row,name))
        for row,name in pending:
            row["value"]["input_motion"] = dict(self.events[name])
