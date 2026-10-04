"""Owned native Maelstrom preview geometry, pixels and input lifecycle."""
import hashlib
import json
import math
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from PIL import Image
from .maelstrom_preview_probe import request, diagnostic_request, typed_request, axis_request, validate_request
from .maelstrom_preview_input import MaelstromPreviewPicker
from .client import OracleClient, OracleProtocolError
from .move_normal_probe import validate_camera
from .translation_input import validate_frame
from .test_twist_preview import screen

ROOT=Path(__file__).parent

class MaelstromPreviewTests(unittest.TestCase):
    def captures(self, name="maelstrom_preview", factory=request):
        fixture=json.loads((ROOT/"fixtures"/(name+".json")).read_text())
        observed=json.loads((ROOT/"observations"/(name+".json")).read_text())
        self.assertEqual(fixture,factory()); validate_request(fixture)
        self.assertEqual([o["id"] for o in fixture["operations"]],[r["id"] for r in observed["results"]])
        return list(zip(fixture["operations"],observed["results"]))

    def test_pending_sources_copy_and_cancel(self):
        for op,row in self.captures():
            with self.subTest(case=op["id"]):
                v=row["value"]; pending=v["pending"]
                validate_frame(pending["frame"]); validate_camera(pending["camera"],pending["frame"])
                self.assertTrue(pending["prompt"].startswith({"First":"Radius","Second":"Second radius"}.get(op["phase"],"Coil angle")))
                self.assertEqual([o for o in pending["objects"] if o["source"] is not None or o["witness"]],v["before"])
                self.assertEqual(len(pending["objects"]),len(v["before"])+(op["phase"]=="Repeat"))
                self.assertEqual(v["success"],op["finish"]=="Click")
                if op["finish"]=="Cancel": self.assertEqual(v["after"],v["before"])
                elif op["copy"]:
                    self.assertEqual([o for o in v["after"] if o["selected"]],v["before"])
                    self.assertEqual(len(v["after"]),2*len(v["before"])+(op["phase"]=="Repeat"))
                else: self.assertNotEqual(v["after"],v["before"])

    def test_png_integrity_and_cubic_preview_pixels(self):
        checked=0
        for op,row in self.captures():
            v=row["value"]; e=v["framebuffer"]; path=ROOT/"observations"/e["path"]
            self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(),e["sha256"])
            with Image.open(path) as bitmap:
                self.assertEqual(list(bitmap.size),e["size"]); self.assertEqual(e["size"],v["pending"]["frame"]["size"])
                bitmap=bitmap.convert("RGB")
                colored=[(i%bitmap.width,i//bitmap.width) for i,c in enumerate(bitmap.getdata()) if max(abs(a-b) for a,b in zip(c,[200,80,60]))<8]
                if op["phase"] in ("First","Second") or op["cursor"]=="Degenerate":
                    self.assertFalse(colored,op["id"]); continue
                if op["shape"]!="Line" or not v["sdk_cubic_preview"]: continue
                matrix=v["pending"]["frame"]["world_to_screen"]
                points=[v["sdk_cubic_preview"][0]["samples"][i] for i in [16,24,32,40,48]]
                hits=sum(bool(colored) and min(math.hypot(x-a,y-b) for a,b in colored)<=3 for x,y in (screen(matrix,p) for p in points))
                self.assertGreaterEqual(hits,4,op["id"]); checked+=1
        self.assertEqual(checked,11)

    def test_native_surface_interior_wires_in_all_display_modes(self):
        checked=0
        for op,row in self.captures():
            if op["shape"]!="Surface": continue
            v=row["value"]
            with Image.open(ROOT/"observations"/v["framebuffer"]["path"]) as bitmap:
                bitmap=bitmap.convert("RGB")
                colored=[(i%bitmap.width,i//bitmap.width) for i,c in enumerate(bitmap.getdata()) if max(abs(a-b) for a,b in zip(c,[200,80,60]))<8]
                matrix=v["pending"]["frame"]["world_to_screen"]
                points=[v["sdk_preview"][0]["samples"][j*9+i] for j in range(1,8) for i in (0,4,8)]
                hits=sum(min(math.hypot(x-a,y-b) for a,b in colored)<=4 for x,y in (screen(matrix,p) for p in points))
                self.assertEqual(hits,len(points),op["id"]); checked+=1
        self.assertEqual(checked,3)

    def test_black_circle_guides_match_accepted_and_pending_radii(self):
        captures=self.captures()
        for index in [1,15,16,25,26,27,28,30]:
            op,row=captures[index]; v=row["value"]; c=v["calibration"]["circle"]
            r0=op["radius0"]
            r1=abs(op["radius1"])
            pending=math.dist(v["calibration"]["valid_point"],c["origin"])
            radii=[pending] if op["phase"]=="First" else [r0,pending if op["phase"]=="Second" else r1]
            matrix=v["pending"]["frame"]["world_to_screen"]
            with Image.open(ROOT/"observations"/v["framebuffer"]["path"]) as bitmap:
                bitmap=bitmap.convert("RGB")
                black=[(i%bitmap.width,i//bitmap.width) for i,c in enumerate(bitmap.getdata()) if max(c)<30]
                for radius in radii:
                    if radius==0:continue
                    for angle in [math.pi/4,3*math.pi/4,5*math.pi/4,7*math.pi/4]:
                        p=[c["origin"][i]+radius*(math.cos(angle)*c["x"][i]+math.sin(angle)*c["y"][i]) for i in range(3)]
                        x,y=screen(matrix,p)
                        if not 0<=x<bitmap.width or not 0<=y<bitmap.height:continue
                        self.assertLessEqual(min(math.hypot(x-a,y-b) for a,b in black),4,op["id"])

    def test_path_and_native_ack_precede_framebuffer_capture(self):
        op=request()["operations"][24]; picker=MaelstromPreviewPicker(dict(protocol_version=1,operations=[op]))
        marker="@maelstrom-preview:"+op["id"]
        with tempfile.TemporaryDirectory() as directory, patch("tools.rhino_oracle.maelstrom_preview_input.subprocess.run") as send, patch("time.monotonic",return_value=2), patch("PIL.ImageGrab.grab") as grab:
            picker.job=Path(directory)
            (picker.job/("maelstrom-preview-"+op["id"]+".json")).write_text(json.dumps(dict(rect=[0,0,708,391],mouse_path=[[100,100]]*81)))
            self.assertFalse(picker.send_input(marker,"100","100","123")); self.assertEqual(send.call_count,2)
            self.assertTrue((picker.job/("maelstrom-preview-path-complete-"+op["id"]+".json")).exists())
            with patch("time.monotonic",return_value=4): self.assertFalse(picker.send_input(marker,"100","100","123"))
            grab.assert_not_called(); self.assertNotIn(marker,picker.images)

    def test_closed_descriptors_and_private_display_are_checked_before_launch(self):
        base=request()["operations"][0]
        for field,bad in [("id","bad\n_Delete"),("shape","Line _Delete"),("copy",1),("radius0",float("nan")),("radius0",True),("radius0",11),("radius0",[0,0,0]),("radius1",float("inf")),("plane",[]),("finish","Enter _Delete"),("cursor","Other"),("view","Front _Delete"),("phase","Repeat"),("aim_angle",True),("aim_angle",451)]:
            with self.subTest(field=field,value=bad),self.assertRaises(ValueError):validate_request(dict(protocol_version=1,operations=[dict(base,**{field:bad})]))
        for scheme,display,headless,iterations in [(None,":201",":201",1),("VibocerosTest",":0",None,1),("VibocerosTest",":201",":201",True),("VibocerosTest",":201",":201",2)]:
            r=request();r["iterations"]=iterations;env={"DISPLAY":display}
            if headless:env["VIBOCEROS_ORACLE_HEADLESS"]=headless
            with patch.dict(os.environ,env,clear=True),patch("tools.rhino_oracle.client._run_logged") as launch:
                with self.assertRaises((ValueError,OracleProtocolError)):OracleClient(settings_scheme=scheme).run_rhino(r,1)
                launch.assert_not_called()

    def test_diagnostic_cases_retain_owned_pixels(self):
        for op,row in self.captures("maelstrom_preview_diagnostic",diagnostic_request):
            v=row["value"]; e=v["framebuffer"]
            self.assertEqual(hashlib.sha256((ROOT/"observations"/e["path"]).read_bytes()).hexdigest(),e["sha256"])

    def test_typed_coordinates_keep_mouse_turns_and_numbers_override_them(self):
        for op,row in self.captures("maelstrom_typed_hover",typed_request):
            v=row["value"];self.assertTrue(v["success"])
            for source,actual,preview in zip(v["before"],v["after"],v["sdk_preview"]):
                src=source["geometry"]["points"][0];dst=actual["geometry"]["points"][0]
                if op["finish"]=="Coordinate":self.assertLess(math.dist(dst,preview["points"][0]),1e-11)
                else:
                    radius=math.hypot(*src[:2]);t=min(1,max(0,(radius-op["radius0"])/(op["radius1"]-op["radius0"])));angle=math.radians(-90)*t*t*(3-2*t)
                    expected=[math.cos(angle)*src[0]-math.sin(angle)*src[1],math.sin(angle)*src[0]+math.cos(angle)*src[1],src[2]]
                    self.assertLess(math.dist(dst,expected),1e-11)
            e=v["framebuffer"]
            self.assertEqual(hashlib.sha256((ROOT/"observations"/e["path"]).read_bytes()).hexdigest(),e["sha256"])

    def test_axis_picks_hide_pending_geometry_and_accept_zero_or_full_turn(self):
        for index,(op,row) in enumerate(self.captures("maelstrom_axis_hover",axis_request)):
            v=row["value"];self.assertTrue(v["success"])
            angle=360 if index==0 else 0
            before=[o for o in v["before"] if not o["witness"]]
            after=[o for o in v["after"] if not o["witness"]]
            for source,actual in zip(before,after):
                src=source["geometry"]["points"][0];dst=actual["geometry"]["points"][0]
                radius=math.hypot(*src[:2]);t=min(1,max(0,(radius-2)/3));a=math.radians(angle)*t*t*(3-2*t)
                expected=[math.cos(a)*src[0]-math.sin(a)*src[1],math.sin(a)*src[0]+math.cos(a)*src[1],src[2]]
                self.assertLess(math.dist(dst,expected),1e-11)
            e=v["framebuffer"];path=ROOT/"observations"/e["path"]
            self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(),e["sha256"])
            if op["cursor"]=="Degenerate":
                with Image.open(path) as bitmap:
                    self.assertFalse(any(max(abs(a-b) for a,b in zip(c[:3],[200,80,60]))<8 for c in bitmap.getdata()))

    def test_provenance_hashes_cover_artifacts_and_helpers(self):
        repo=ROOT.parents[1]; provenance=json.loads((repo/"docs/maelstrom-preview-provenance.json").read_text())
        for path,digest in provenance["sha256"].items():self.assertEqual(hashlib.sha256((repo/path).read_bytes()).hexdigest(),digest,path)

if __name__=="__main__":unittest.main()
