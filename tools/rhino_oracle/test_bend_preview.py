"""Private native Bend preview evidence and owned pointer acknowledgement."""
import copy
import hashlib
import json
import math
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from PIL import Image
from .bend_preview_probe import request, followup_request, validate_request
from .bend_preview_input import BendPreviewPicker
from .client import OracleClient, OracleProtocolError
from .move_normal_probe import validate_camera
from .translation_input import validate_frame
from .test_twist_preview import screen

ROOT = Path(__file__).parent


class BendPreviewTests(unittest.TestCase):
    def captures(self):
        result = []
        for name, factory in (("bend_preview", request), ("bend_preview_followup", followup_request)):
            fixture = json.loads((ROOT / "fixtures" / (name + ".json")).read_text())
            observed = json.loads((ROOT / "observations" / (name + ".json")).read_text())
            self.assertEqual(fixture, factory())
            validate_request(fixture)
            self.assertEqual([op["id"] for op in fixture["operations"]], [row["id"] for row in observed["results"]])
            result.extend(zip(fixture["operations"], observed["results"]))
        return result

    def test_sources_remain_unchanged_until_click_or_cancel(self):
        for op, row in self.captures():
            with self.subTest(case=op["id"]):
                v = row["value"]
                pending = v["pending"]
                validate_frame(pending["frame"])
                validate_camera(pending["camera"], pending["frame"])
                self.assertTrue(pending["prompt"].startswith("Point to bend through"))
                self.assertEqual([o for o in pending["objects"] if o["source"] is not None or o["witness"]], v["before"])
                self.assertEqual(len(pending["objects"]), len(v["before"]) + (op["phase"] == "Repeat"))
                if op["finish"] == "Cancel":
                    self.assertFalse(v["success"])
                    self.assertEqual(v["after"], v["before"])
                else:
                    self.assertTrue(v["success"])
                    self.assertNotEqual(v["after"], v["before"])
                    if op["copy"]:
                        self.assertEqual([o for o in v["after"] if o["selected"]], v["before"])
                        self.assertEqual(len(v["after"]), 2 * len(v["before"]) + (op["phase"] == "Repeat"))

    def test_pngs_are_complete_and_line_cages_follow_public_cubic_quick_previews(self):
        checked = 0
        for op, row in self.captures():
            v = row["value"]; e = v["framebuffer"]
            data = (ROOT / "observations" / e["path"]).read_bytes()
            self.assertEqual(hashlib.sha256(data).hexdigest(), e["sha256"], op["id"])
            with Image.open(ROOT / "observations" / e["path"]) as bitmap:
                self.assertEqual(list(bitmap.size), e["size"])
                self.assertEqual(e["size"], v["pending"]["frame"]["size"])
                bitmap = bitmap.convert("RGB")
                colored = [(i % bitmap.width, i // bitmap.width) for i, c in enumerate(bitmap.getdata()) if max(abs(a - b) for a, b in zip(c, [200, 80, 60])) < 25]
                if op["cursor"] == "Degenerate":
                    self.assertFalse(any(max(abs(a-b) for a,b in zip(c,[200,80,60]))<8 for c in bitmap.getdata()), op["id"])
                    continue
                if op["shape"] != "Line" or not v["sdk_cubic_preview"]:
                    continue
                self.assertTrue(colored, op["id"])
                matrix = v["pending"]["frame"]["world_to_screen"]
                hits = 0
                for i in [16, 24, 32, 40, 48]:
                    xy = screen(matrix, v["sdk_cubic_preview"][0]["samples"][i])
                    hits += min(math.hypot(xy[0]-x, xy[1]-y) for x,y in colored) <= 3
                self.assertGreaterEqual(hits, 4, op["id"])
                checked += 1
        self.assertGreaterEqual(checked, 8)

    def test_native_surface_quick_preview_wires_have_pixel_witnesses(self):
        checked = 0
        for op, row in self.captures():
            v = row["value"]
            if op["shape"] != "Surface" or op["cursor"] == "Degenerate":
                continue
            with Image.open(ROOT / "observations" / v["framebuffer"]["path"]) as bitmap:
                bitmap = bitmap.convert("RGB")
                colored = [(i % bitmap.width, i // bitmap.width) for i, c in enumerate(bitmap.getdata()) if max(abs(a-b) for a,b in zip(c,[200,80,60])) < 8]
                self.assertTrue(colored, op["id"])
                matrix=v["pending"]["frame"]["world_to_screen"]
                points=[v["sdk_preview"][0]["samples"][j*9+i] for j in range(1,8) for i in ((0,4,8) if op["display_mode"]=="Wireframe" else (0,8))]
                hits=sum(min(math.hypot(x-a,y-b) for a,b in colored) <= (4 if op["display_mode"]=="Wireframe" else 2) for x,y in (screen(matrix,p) for p in points))
                self.assertEqual(hits, len(points), op["id"])
                checked+=1
        self.assertEqual(checked, 6)

    def test_first_copy_capture_waits_for_native_motion_before_collecting_pixels(self):
        op=request()["operations"][17]
        picker=BendPreviewPicker(dict(protocol_version=1,operations=[op]))
        marker="@bend-preview:"+op["id"]
        with tempfile.TemporaryDirectory() as directory, patch("tools.rhino_oracle.mirror_preview_input.subprocess.run") as send, patch("time.monotonic",return_value=2), patch("PIL.ImageGrab.grab") as grab:
            picker.job=Path(directory)
            (picker.job/("bend-preview-"+op["id"]+".json")).write_text(json.dumps(dict(valid_screen=[100,100],rect=[0,0,708,391])))
            self.assertFalse(picker.send_input(marker,"100","100","123"))
            with patch("time.monotonic",return_value=4):
                self.assertFalse(picker.send_input(marker,"100","100","123"))
            self.assertEqual(send.call_count,2)
            grab.assert_not_called()
            self.assertNotIn(marker,picker.images)

    def test_closed_descriptors_and_private_display_are_checked_before_launch(self):
        base=request()["operations"][0]
        for field, bad in [("id","bad\n_Delete"),("shape","Surface _Delete"),("copy",1),("angle",float("nan")),("angle",True),("angle",361),("finish","Enter _Delete"),("cursor","Other"),("view","Front _Delete"),("phase","Repeat")]:
            with self.subTest(field=field,value=bad),self.assertRaises(ValueError):
                validate_request(dict(protocol_version=1,operations=[dict(base,**{field:bad})]))
        for scheme, display, headless, iterations in [(None,":201",":201",1),("VibocerosTest",":0",None,1),("VibocerosTest",":201",":201",True),("VibocerosTest",":201",":201",2)]:
            r=request();r["iterations"]=iterations
            env={"DISPLAY":display}
            if headless:env["VIBOCEROS_ORACLE_HEADLESS"]=headless
            with patch.dict(os.environ,env,clear=True),patch("tools.rhino_oracle.client._run_logged") as launch:
                with self.assertRaises((ValueError,OracleProtocolError)):
                    OracleClient(settings_scheme=scheme).run_rhino(r,1)
                launch.assert_not_called()

    def test_initial_cursor_race_is_preserved_as_diagnostic_evidence(self):
        initial=json.loads((ROOT/"observations/bend_preview_initial.json").read_text())
        good=json.loads((ROOT/"observations/bend_preview.json").read_text())
        old=initial["results"][17]["value"]
        current=good["results"][17]["value"]
        for v, expect in ((old,False),(current,True)):
            with Image.open(ROOT/"observations"/v["framebuffer"]["path"]) as bitmap:
                colored=any(max(abs(a-b) for a,b in zip(c,[200,80,60]))<8 for c in bitmap.convert("RGB").getdata())
                self.assertEqual(colored,expect)
        self.assertTrue(old["success"])
        self.assertTrue(current["success"])
        self.assertEqual(old["after"],current["after"])

    def test_provenance_hashes_cover_artifacts_and_capture_helpers(self):
        repo=ROOT.parents[1]
        provenance=json.loads((repo/"docs/bend-preview-provenance.json").read_text())
        for path, digest in provenance["sha256"].items():
            self.assertEqual(hashlib.sha256((repo/path).read_bytes()).hexdigest(),digest,path)


if __name__ == "__main__":
    unittest.main()
