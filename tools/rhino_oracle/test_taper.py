"""Bounded Taper recipes, private launches, and raw native evidence."""
import hashlib
import json
import os
from pathlib import Path
import unittest
from unittest.mock import patch

from . import taper_probe
from .client import OracleClient, OracleError, OracleProtocolError

ROOT = Path(__file__).parent


class TaperTests(unittest.TestCase):
    def test_factories_match_retained_inputs_and_closed_descriptors(self):
        for name, factory in [("taper_points", taper_probe.request), ("taper_command_points", taper_probe.command_request),
                              ("taper_boundary_points", taper_probe.boundary_request)]:
            request = factory()
            self.assertEqual(request, json.loads((ROOT/"fixtures"/(name+".json")).read_text()))
            for op in request["operations"]:
                taper_probe.validate(op)
            base = request["operations"][0]
            for change in [dict(id="bad\n_Delete"), dict(start_radius=True), dict(end_radius="1 _Delete"),
                           dict(start_radius=float("nan")), dict(end_radius=float("inf")),
                           dict(start_radius=1e7), dict(flat=1), dict(infinite="Yes"),
                           dict(start=[True,0,0]), dict(end=[0,0,float("inf")]),
                           dict(points=[]), dict(points=[[0,0,0]]*257), dict(points=[[0,0]]),
                           dict(points=[[0,0,1e7]]), dict(extra=True)]:
                with self.subTest(change=change), self.assertRaises(ValueError):
                    taper_probe.validate(dict(base,**change))

    def test_private_display_scheme_and_one_command_iteration_are_required(self):
        for scheme, iterations in [(None,1),("VibocerosOracleTest",True),("VibocerosOracleTest",2)]:
            request=taper_probe.command_request()
            request["iterations"]=iterations
            with patch.dict(os.environ,{"DISPLAY":":101","VIBOCEROS_ORACLE_HEADLESS":":101"}), \
                 patch("tools.rhino_oracle.client._run_logged") as launch, self.assertRaises(OracleProtocolError):
                OracleClient(launcher="/bin/true",settings_scheme=scheme).run_rhino(request)
            launch.assert_not_called()
        with patch.dict(os.environ,{"DISPLAY":":0","VIBOCEROS_ORACLE_HEADLESS":":101"}), \
             patch("tools.rhino_oracle.client._run_logged") as launch, self.assertRaisesRegex(OracleError,"dedicated Xvfb"):
            OracleClient(launcher="/bin/true").run_rhino(taper_probe.request())
        launch.assert_not_called()

    def test_native_rows_have_complete_samples_and_command_history(self):
        for name,factory in [("taper_points",taper_probe.request),("taper_command_points",taper_probe.command_request),
                             ("taper_boundary_points",taper_probe.boundary_request)]:
            request=factory()
            response=json.loads((ROOT/"observations"/(name+".json")).read_text())
            self.assertEqual(response["engine"],"rhino")
            self.assertEqual(response["engine_version"],"8.32.26160.13001")
            self.assertEqual(len(request["operations"]),len(response["results"]))
            for op,row in zip(request["operations"],response["results"]):
                self.assertEqual(op["id"],row["id"])
                value=row["value"]
                if op["op"]=="taper_points":
                    self.assertIs(type(value["valid"]),bool)
                    self.assertEqual(len(value["points"]),len(op["points"]))
                    if not value["valid"]:
                        self.assertEqual(value["points"],op["points"])
                else:
                    self.assertTrue(value["success"],op["id"])
                    self.assertIn("_Taper",value["history"])
                    self.assertEqual([o["point"] for o in value["before"]],op["points"])
                    self.assertEqual(len(value["after"]),len(op["points"])*(2 if op["copy"] else 1))
                    self.assertEqual([o["point"] for o in value["undo"]],op["points"])
                    self.assertEqual([o["point"] for o in value["redo"]],[o["point"] for o in value["after"]])
                    self.assertEqual(next(e["result"] for e in value["events"] if e["name"]=="Taper"),"Success")

    def test_provenance_hashes_cover_native_artifacts(self):
        repo=ROOT.parents[1]
        provenance=json.loads((repo/"docs/taper-provenance.json").read_text())
        for path,digest in provenance["sha256"].items():
            self.assertEqual(hashlib.sha256((repo/path).read_bytes()).hexdigest(),digest,path)

    def test_nested_script_undo_diagnostic_retains_unmodified_geometry(self):
        initial=json.loads((ROOT/"observations/taper_command_points_initial.json").read_text())
        current=json.loads((ROOT/"observations/taper_command_points.json").read_text())
        self.assertEqual(len(initial["results"]),len(current["results"]))
        for old,new in zip(initial["results"],current["results"]):
            self.assertEqual(old["id"],new["id"])
            a,b=old["value"],new["value"]
            self.assertTrue(a["success"])
            self.assertEqual([o["point"] for o in a["after"]],[o["point"] for o in b["after"]])
            self.assertEqual([o["point"] for o in a["undo"]],[o["point"] for o in a["after"]])
            self.assertNotEqual([o["point"] for o in a["undo"]],[o["point"] for o in a["before"]])


if __name__=="__main__":
    unittest.main()
