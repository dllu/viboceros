"""Capture pending Bend pixels using the private-display owned cursor picker."""

import base64
import hashlib
import json
import subprocess
import time

from .client import OracleProtocolError
from .mirror_preview_input import CursorPreviewPicker
from .bend_preview_probe import validate_request


class BendPreviewPicker(CursorPreviewPicker):
    def __init__(self, request):
        super().__init__(request, "bend", validate_request)

    def send_input(self, name, x, y, window):
        if name in self.moved and name not in self.images:
            ready = self.job / ("bend-preview-ready-" + self.cases[name]["id"] + ".json")
            needs_degenerate_move = self.cases[name]["cursor"] == "Degenerate" and name not in self.degenerate
            if not ready.exists() and not needs_degenerate_move:
                previous, px, py, moved_at = self.moved[name]
                if (window, x, y) != (previous, px, py):
                    raise OracleProtocolError("owned Bend cursor changed before acknowledgement")
                if time.monotonic() - moved_at >= 1.0:
                    subprocess.run(["xdotool", "windowactivate", "--sync", window,
                                    "mousemove", str(int(x) + 1), y,
                                    "mousemove", x, y], check=True, timeout=10)
                    self.moved[name] = (window, x, y, time.monotonic())
                return False
        return super().send_input(name, x, y, window)


def main():
    import argparse
    from pathlib import Path
    from .client import OracleClient, load_request

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("request", type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--scheme", required=True)
    parser.add_argument("--timeout", type=float, default=300.0)
    args = parser.parse_args()
    request = load_request(args.request)
    validate_request(request)
    response = OracleClient(settings_scheme=args.scheme).run_rhino(request, args.timeout)
    folder = args.output.parent / args.output.stem
    folder.mkdir(parents=True, exist_ok=True)
    for row in response["results"]:
        evidence = row["value"]["framebuffer"]
        data = base64.b64decode(evidence.pop("png_base64"), validate=True)
        if hashlib.sha256(data).hexdigest() != evidence["sha256"]:
            raise OracleProtocolError("Bend preview PNG checksum differs")
        filename = row["id"] + ".png"
        (folder / filename).write_bytes(data)
        evidence["path"] = folder.name + "/" + filename
    args.output.write_text(json.dumps(response, separators=(",", ":"), allow_nan=False) + "\n")
    print("Captured %d native Bend previews" % len(response["results"]))


if __name__ == "__main__":
    main()
