"""Raw private-display evidence and bounded Twist pointer instrumentation."""

import copy
import hashlib
import json
import math
import os
import struct
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from PIL import Image
from .client import OracleClient, OracleProtocolError
from .move_normal_probe import validate_camera
from .translation_input import validate_frame
from .twist_preview_probe import request, recipe, validate_request
from .twist_preview_input import TwistPreviewPicker

ROOT = Path(__file__).parent


def screen(matrix, point):
    result = [sum(x * y for x, y in zip(row[:3], point)) + row[3] for row in matrix]
    return [round(result[i] / result[3]) for i in range(2)]


class TwistPreviewTests(unittest.TestCase):
    def captures(self):
        fixture = json.loads((ROOT / "fixtures/twist_preview.json").read_text())
        observed = json.loads((ROOT / "observations/twist_preview.json").read_text())
        self.assertEqual(fixture, request())
        validate_request(fixture)
        self.assertEqual(len(fixture["operations"]), 32)
        self.assertEqual(
            [op["id"] for op in fixture["operations"]],
            [row["id"] for row in observed["results"]],
        )
        return list(zip(fixture["operations"], observed["results"]))

    def test_native_sources_stay_unchanged_until_accept_or_cancel(self):
        for op, row in self.captures():
            with self.subTest(case=op["id"]):
                v = row["value"]
                pending = v["pending"]
                validate_frame(pending["frame"])
                validate_camera(pending["camera"], pending["frame"])
                originals = [
                    o for o in pending["objects"] if o["selected"] or o["witness"]
                ]
                self.assertEqual(originals, v["before"])
                self.assertEqual(
                    len(pending["objects"]),
                    len(v["before"]) + (op["phase"] == "Repeat"),
                )
                if op["finish"] == "Cancel":
                    self.assertFalse(v["success"])
                    self.assertEqual(v["after"], v["before"])
                else:
                    self.assertTrue(v["success"])
                    self.assertIn("Command" if op["copy"] else "Morphed", v["history"])
                    self.assertNotEqual(v["after"], v["before"])
                    if op["copy"]:
                        self.assertEqual(
                            [o for o in v["after"] if o["selected"]], v["before"]
                        )
                        self.assertEqual(
                            len(v["after"]),
                            2 * len(v["before"]) + (op["phase"] == "Repeat"),
                        )

    def test_png_hash_dimensions_and_cubic_preview_pixel_witnesses(self):
        for op, row in self.captures():
            v = row["value"]
            e = v["framebuffer"]
            data = (ROOT / "observations" / e["path"]).read_bytes()
            self.assertEqual(hashlib.sha256(data).hexdigest(), e["sha256"], op["id"])
            self.assertEqual(list(struct.unpack(">II", data[16:24])), e["size"])
            self.assertEqual(e["size"], v["pending"]["frame"]["size"])
            rect = v["calibration"]["rect"]
            self.assertEqual([rect[2] - rect[0], rect[3] - rect[1]], e["size"])
            if op["shape"] != "Line" or op["rigid"] or op["phase"] == "Reference":
                continue
            with Image.open(ROOT / "observations" / e["path"]) as bitmap:
                bitmap = bitmap.convert("RGB")
                hits = 0
                for i in [16, 24, 32, 40, 48]:
                    pt = v["sdk_cubic_preview"][0]["samples"][i]
                    x, y = screen(v["pending"]["frame"]["world_to_screen"], pt)
                    hits += any(
                        abs(r - 200) < 25 and abs(g - 80) < 25 and abs(b - 60) < 25
                        for r, g, b in bitmap.crop(
                            (x - 3, y - 3, x + 4, y + 4)
                        ).getdata()
                    )
                self.assertGreaterEqual(hits, 4, op["id"])
        # Selected shaded faces retain the source color; only wires highlight.
        for i in [8, 25]:
            _, row = self.captures()[i]
            v = row["value"]
            with Image.open(ROOT / "observations" / v["framebuffer"]["path"]) as bitmap:
                xy = screen(v["pending"]["frame"]["world_to_screen"], [2.4, 0.4, 10.0])
                self.assertEqual(
                    bitmap.convert("RGB").getpixel(tuple(xy)), (206, 139, 127)
                )

    def test_surface_and_brep_quick_preview_paths_match_native_pixels(self):
        def evaluate(patch, u, v):
            du, dv = patch["degree"]
            nu, nv = patch["control_count"]
            self.assertEqual([nu, nv], [du + 1, dv + 1])
            q = [0.0, 0.0, 0.0, 0.0]
            for j in range(nv):
                for i in range(nu):
                    c = patch["control_points"][j * nu + i]
                    weight = (
                        math.comb(du, i)
                        * u**i
                        * (1 - u) ** (du - i)
                        * math.comb(dv, j)
                        * v**j
                        * (1 - v) ** (dv - j)
                        * c["weight"]
                    )
                    for k in range(4):
                        q[k] += weight * (c["point"][k] if k < 3 else 1.0)
            return [q[k] / q[3] for k in range(3)]

        for op, row in self.captures():
            if op["shape"] not in ("Surface", "Box") or op["phase"] == "Reference":
                continue
            v = row["value"]
            matrix = v["pending"]["frame"]["world_to_screen"]
            with Image.open(ROOT / "observations" / v["framebuffer"]["path"]) as bitmap:
                bitmap = bitmap.convert("RGB")
                colored = [
                    (i % bitmap.width, i // bitmap.width)
                    for i, c in enumerate(bitmap.getdata())
                    if max(abs(a - b) for a, b in zip(c, [200, 80, 60])) < 8
                ]
                self.assertTrue(colored)
                if op["shape"] == "Surface":
                    points = [
                        v["sdk_preview"][0]["samples"][9 * j + i]
                        for j in range(1, 8)
                        for i in (0, 4, 8)
                    ]
                else:
                    points = [
                        evaluate(
                            patch,
                            *((i / 16.0, fixed) if axis == 0 else (fixed, i / 16.0)),
                        )
                        for patch in v["sdk_preview"][0]["surfaces"]
                        for fixed in (0.0, 0.5, 1.0)
                        for axis in (0, 1)
                        for i in range(1, 16)
                    ]
                hits = sum(
                    min(math.hypot(x - a, y - b) for a, b in colored) <= 2.0
                    for x, y in (screen(matrix, p) for p in points)
                )
                self.assertGreaterEqual(
                    hits,
                    19 if op["shape"] == "Surface" else 0.85 * len(points),
                    op["id"],
                )
        fixture = json.loads((ROOT / "fixtures/twist_preview_edges.json").read_text())
        validate_request(fixture)
        self.assertEqual(fixture["operations"], [request()["operations"][2]])
        observed = json.loads(
            (ROOT / "observations/twist_preview_edges.json").read_text()
        )
        for row in observed["results"]:
            v = row["value"]
            edges = v["sdk_preview"][0]["edges"]
            self.assertEqual(len(edges), 12)
            self.assertTrue(all(e["definition"]["degree"] == 3 for e in edges))
            e = v["framebuffer"]
            self.assertEqual(
                hashlib.sha256(
                    (ROOT / "observations" / e["path"]).read_bytes()
                ).hexdigest(),
                e["sha256"],
            )

    def test_independent_cubic_control_map_and_actual_450_degree_completion(self):
        for op, row in self.captures():
            if op["shape"] != "Line" or op["rigid"]:
                continue
            v = row["value"]
            native = v["sdk_cubic_preview"][0]
            start = v["before"][0]["geometry"]["samples"][0]
            end = v["before"][0]["geometry"]["samples"][64]
            self.assertEqual(native["definition"]["degree"], 3)
            for i, control in enumerate(native["definition"]["control_points"]):
                p = [a + (b - a) * i / 3 for a, b in zip(start, end)]
                t = max(0.0, min(1.0, p[2] / 10.0))
                theta = math.radians(recipe(op)["angle"]) * t * t * (3 - 2 * t)
                expected = [
                    p[0] * math.cos(theta) - p[1] * math.sin(theta),
                    p[0] * math.sin(theta) + p[1] * math.cos(theta),
                    p[2],
                ]
                for a, b in zip(control["point"], expected):
                    self.assertAlmostEqual(a, b, delta=1e-11, msg=op["id"])
        op, row = self.captures()[31]
        self.assertEqual(op["phase"], "Wrap")
        before = row["value"]["before"][0]["geometry"]["samples"][16]
        after = row["value"]["after"][0]["geometry"]["samples"][16]
        rotation = math.degrees(
            math.atan2(after[1], after[0]) - math.atan2(before[1], before[0])
        )
        t = before[2] / 10.0
        self.assertAlmostEqual(rotation / (t * t * (3 - 2 * t)), 450.0, delta=1e-8)

    def test_probe_rejects_unbounded_and_foreign_recipes_before_launch(self):
        op = request()["operations"][0]
        for changes in [
            dict(script="_Delete"),
            dict(id="../bad"),
            dict(command="Delete"),
            dict(copy=1),
            dict(rigid="No"),
            dict(phase="Repeat"),
            dict(phase="Reference"),
            dict(shape="Box", preserve=True),
            dict(view="Other"),
            dict(display_mode="Other"),
            dict(cursor="Degenerate"),
        ]:
            with self.assertRaises(ValueError):
                validate_request(
                    dict(protocol_version=1, operations=[dict(op, **changes)])
                )
        for invalid in [
            None,
            [],
            dict(protocol_version=True, operations=[op]),
            dict(protocol_version=1, iterations=2, operations=[op]),
            dict(protocol_version=1, operations=[op, op]),
            dict(
                protocol_version=1, operations=[dict(op, id=str(i)) for i in range(33)]
            ),
        ]:
            with self.assertRaises(ValueError):
                validate_request(invalid)
        with patch.dict(
            os.environ, {"DISPLAY": ":101", "VIBOCEROS_ORACLE_HEADLESS": ":101"}
        ), patch("tools.rhino_oracle.client._run_logged") as launch, self.assertRaises(
            OracleProtocolError
        ):
            OracleClient(launcher="/bin/true").run_rhino(request())
        launch.assert_not_called()

    def test_owned_path_must_finish_and_snapshot_must_acknowledge_before_final_input(
        self,
    ):
        op = request()["operations"][0]
        name = "@twist-preview:" + op["id"]
        with tempfile.TemporaryDirectory() as folder, patch(
            "tools.rhino_oracle.twist_preview_input.subprocess.run"
        ) as send:
            picker = TwistPreviewPicker(dict(protocol_version=1, operations=[op]))
            picker.job = Path(folder)
            metadata = picker.job / ("twist-preview-" + op["id"] + ".json")
            metadata.write_text(
                json.dumps(dict(rect=[0, 0, 300, 200], mouse_path=[[100, 120]] * 17))
            )
            self.assertFalse(picker.send_input(name, "100", "120", "owned"))
            commands = send.call_args_list[0].args[0]
            self.assertEqual(
                commands[:4], ["xdotool", "windowactivate", "--sync", "owned"]
            )
            self.assertEqual(commands.count("mousemove"), 17)
            self.assertTrue(
                (
                    picker.job / ("twist-preview-path-complete-" + op["id"] + ".json")
                ).exists()
            )
            self.assertEqual(picker.images, {})
            self.assertFalse(picker.send_input(name, "100", "120", "owned"))
            before = send.call_count
            with self.assertRaises(OracleProtocolError):
                picker.send_input(name, "100", "120", "foreign")
            self.assertEqual(send.call_count, before)
            picker.images[name] = {"recorded": True}
            self.assertFalse(picker.send_input(name, "100", "120", "owned"))
            ready = picker.job / ("twist-preview-ready-" + op["id"] + ".json")
            ready.write_text(json.dumps("foreign"))
            with self.assertRaises(OracleProtocolError):
                picker.send_input(name, "100", "120", "owned")
            self.assertEqual(send.call_count, before)
            ready.write_text(json.dumps(op["id"]))
            self.assertTrue(picker.send_input(name, "100", "120", "owned"))
            self.assertEqual(send.call_args.args[0][-2:], ["click", "1"])
            response = {"results": [{"id": op["id"], "value": {"raw": "preserved"}}]}
            original = copy.deepcopy(response)
            with self.assertRaises(OracleProtocolError):
                picker.record_diagnostics(response)
            self.assertEqual(response, original)

    def test_pointer_paths_cannot_leave_the_owned_capture_rectangle(self):
        op = request()["operations"][0]
        name = "@twist-preview:" + op["id"]
        for path in [[[10, 20]] * 16, [[400, 20]] * 17, [[True, 20]] * 17]:
            with tempfile.TemporaryDirectory() as folder, patch(
                "tools.rhino_oracle.twist_preview_input.subprocess.run"
            ) as send:
                picker = TwistPreviewPicker(dict(protocol_version=1, operations=[op]))
                picker.job = Path(folder)
                (picker.job / ("twist-preview-" + op["id"] + ".json")).write_text(
                    json.dumps(dict(rect=[0, 0, 300, 200], mouse_path=path))
                )
                with self.assertRaises(OracleProtocolError):
                    picker.send_input(name, "100", "120", "owned")
                send.assert_not_called()


if __name__ == "__main__":
    unittest.main()
