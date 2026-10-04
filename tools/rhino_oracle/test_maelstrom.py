"""Bounded Maelstrom recipes, private launches, and raw native evidence."""
import hashlib
import json
import os
from pathlib import Path
import unittest
from unittest.mock import patch

from . import maelstrom_probe
from .client import OracleClient, OracleError, OracleProtocolError

ROOT = Path(__file__).parent
FACTORIES = [("maelstrom_points",maelstrom_probe.request),("maelstrom_followup",maelstrom_probe.followup_request),
             ("maelstrom_profile_points",maelstrom_probe.profile_request),("maelstrom_threshold_points",maelstrom_probe.threshold_request)]


class MaelstromTests(unittest.TestCase):
    def test_factories_match_retained_inputs_and_closed_descriptors(self):
        for name, factory in FACTORIES:
            request = factory()
            self.assertEqual(request, json.loads((ROOT/"fixtures"/(name+".json")).read_text()))
            for op in request["operations"]:
                maelstrom_probe.validate(op)
            base = request["operations"][0]
            for change in [dict(id="bad\n_Delete"), dict(radius0=True), dict(radius1="1 _Delete"),
                           dict(radius0=float("nan")), dict(radius1=float("inf")),
                           dict(radius0=1e7), dict(angle_radians=True), dict(angle_radians=float("nan")),
                           dict(origin=[True,0,0]), dict(normal=[0,0,float("inf")]),
                           dict(points=[]), dict(points=[[0,0,0]]*257), dict(points=[[0,0]]),
                           dict(points=[[0,0,1e7]]), dict(extra=True)]:
                with self.subTest(change=change), self.assertRaises(ValueError):
                    maelstrom_probe.validate(dict(base,**change))

    def test_private_display_scheme_and_one_command_iteration_are_required(self):
        for scheme, iterations in [(None,1),("VibocerosOracleTest",True),("VibocerosOracleTest",2)]:
            request=maelstrom_probe.followup_request()
            request["iterations"]=iterations
            with patch.dict(os.environ,{"DISPLAY":":101","VIBOCEROS_ORACLE_HEADLESS":":101"}), \
                 patch("tools.rhino_oracle.client._run_logged") as launch, self.assertRaises(OracleProtocolError):
                OracleClient(launcher="/bin/true",settings_scheme=scheme).run_rhino(request)
            launch.assert_not_called()
        with patch.dict(os.environ,{"DISPLAY":":0","VIBOCEROS_ORACLE_HEADLESS":":101"}), \
             patch("tools.rhino_oracle.client._run_logged") as launch, self.assertRaisesRegex(OracleError,"dedicated Xvfb"):
            OracleClient(launcher="/bin/true").run_rhino(maelstrom_probe.request())
        launch.assert_not_called()

    def test_native_rows_have_complete_samples_and_command_history(self):
        for name,factory in FACTORIES:
            request=factory()
            response=json.loads((ROOT/"observations"/(name+".json")).read_text())
            self.assertEqual(response["engine"],"rhino")
            self.assertEqual(response["engine_version"],"8.32.26160.13001")
            self.assertEqual(len(request["operations"]),len(response["results"]))
            for op,row in zip(request["operations"],response["results"]):
                self.assertEqual(op["id"],row["id"])
                value=row["value"]
                if op["op"]=="maelstrom_points":
                    self.assertIs(type(value["valid"]),bool)
                    self.assertEqual(len(value["points"]),len(op["points"]))
                    if not value["valid"]:
                        self.assertEqual(value["points"],op["points"])
                else:
                    self.assertTrue(value["success"],op["id"])
                    self.assertIn("_Maelstrom",value["history"])
                    self.assertEqual([o["point"] for o in value["before"]],op["points"])
                    self.assertEqual(len(value["after"]),len(op["points"])*(2 if op["copy"] else 1))
                    if value["undo"] is not None:
                        self.assertEqual([o["point"] for o in value["undo"]],op["points"])
                    if value["redo"] is not None:
                        self.assertEqual([o["point"] for o in value["redo"]],[o["point"] for o in value["after"]])
                    self.assertEqual(next(e["result"] for e in value["events"] if e["name"]=="Maelstrom"),"Success")

    def test_provenance_hashes_cover_native_artifacts(self):
        repo=ROOT.parents[1]
        provenance=json.loads((repo/"docs/maelstrom-provenance.json").read_text())
        for path,digest in provenance["sha256"].items():
            self.assertEqual(hashlib.sha256((repo/path).read_bytes()).hexdigest(),digest,path)


if __name__=="__main__":
    unittest.main()
