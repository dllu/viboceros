"""Capture owned maelstrom previews and preserve raw PNG evidence."""

import base64
import hashlib
import json
import subprocess
import time
from .client import OracleProtocolError
from .mirror_preview_input import CursorPreviewPicker
from .maelstrom_preview_probe import validate_request


class MaelstromPreviewPicker(CursorPreviewPicker):
    def __init__(self, request):
        super().__init__(request, "maelstrom", validate_request)

    def send_input(self, name, x, y, window):
        if name in self.images and self.cases[name]["finish"] in ("Coordinate", "Number", "Axis"):
            case = self.cases[name]
            previous, px, py, _ = self.moved[name]
            if (window, x, y) != (previous, px, py):
                raise OracleProtocolError("owned Maelstrom target changed before typing")
            ready = self.job / ("maelstrom-preview-ready-" + case["id"] + ".json")
            if not ready.exists():
                return False
            if json.loads(ready.read_text()) != case["id"]:
                raise OracleProtocolError("foreign Maelstrom snapshot acknowledgement")
            # The owned worker sends bounded input via RhinoApp.SendKeystrokes
            # after observing the PNG receipt. Linux key synthesis is unreliable
            # in this Wine GetAngle loop.
            return True
        if name in self.cases and name not in self.moved:
            metadata = json.loads(
                (
                    self.job / ("maelstrom-preview-" + self.cases[name]["id"] + ".json")
                ).read_text()
            )
            path = metadata["mouse_path"]
            rect = metadata["rect"]
            if (
                not isinstance(rect, list)
                or len(rect) != 4
                or any(type(v) is not int for v in rect)
                or not 0 <= rect[0] < rect[2] <= 1920
                or not 0 <= rect[1] < rect[3] <= 1080
                or not isinstance(path, list)
                or len(path) != (81 if self.cases[name]["phase"] == "Wrap" else 17)
                or any(
                    not isinstance(p, list)
                    or len(p) != 2
                    or any(type(v) is not int for v in p)
                    or not rect[0] <= p[0] < rect[2]
                    or not rect[1] <= p[1] < rect[3]
                    for p in path
                )
            ):
                raise OracleProtocolError("invalid owned Maelstrom pointer path")
            argv = ["xdotool", "windowactivate", "--sync", window]
            for px, py in path:
                argv += ["mousemove", str(px), str(py), "sleep", "0.025"]
            subprocess.run(argv, check=True, timeout=10)
            (
                self.job
                / ("maelstrom-preview-path-complete-" + self.cases[name]["id"] + ".json")
            ).write_text(json.dumps(self.cases[name]["id"]))
            subprocess.run(
                [
                    "xdotool",
                    "mousemove",
                    str(int(x) + 1),
                    y,
                    "sleep",
                    "0.025",
                    "mousemove",
                    x,
                    y,
                ],
                check=True,
                timeout=10,
            )
            if self.cases[name]["cursor"] == "Degenerate":
                self.degenerate.add(name)
            self.moved[name] = (window, x, y, time.monotonic())
            return False
        if name in self.cases and name in self.moved and name not in self.images:
            previous, px, py, moved_at = self.moved[name]
            if (window, x, y) != (previous, px, py):
                raise OracleProtocolError(
                    "owned maelstrom target changed while awaiting native snapshot"
                )
            case = self.cases[name]
            # The generic picker first moves to a valid point for degenerate cases.
            # Let it deliver the second motion before waiting for the snapshot.
            if case["cursor"] != "Degenerate" or name in self.degenerate:
                ready = self.job / ("maelstrom-preview-ready-" + case["id"] + ".json")
                if not ready.exists():
                    if time.monotonic() - moved_at >= 1.0:
                        subprocess.run(
                            [
                                "xdotool",
                                "windowactivate",
                                "--sync",
                                window,
                                "mousemove",
                                str(int(x) + 1),
                                y,
                                "mousemove",
                                x,
                                y,
                            ],
                            check=True,
                            timeout=10,
                        )
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
    response = OracleClient(settings_scheme=args.scheme).run_rhino(
        request, args.timeout
    )
    folder = args.output.parent / args.output.stem
    folder.mkdir(parents=True, exist_ok=True)
    for row in response["results"]:
        evidence = row["value"]["framebuffer"]
        data = base64.b64decode(evidence.pop("png_base64"), validate=True)
        if hashlib.sha256(data).hexdigest() != evidence["sha256"]:
            raise OracleProtocolError("maelstrom preview PNG checksum differs")
        filename = row["id"] + ".png"
        (folder / filename).write_bytes(data)
        evidence["path"] = folder.name + "/" + filename
    args.output.write_text(json.dumps(response, indent=2, allow_nan=False) + "\n")
    print("Captured %d native maelstrom previews" % len(response["results"]))


if __name__ == "__main__":
    main()
