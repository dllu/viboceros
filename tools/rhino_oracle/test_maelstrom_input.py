import copy
import hashlib
import json
import math
import os
from pathlib import Path
from unittest import TestCase, mock

from .client import OracleClient, OracleProtocolError
from .maelstrom_input_probe import request, validate

ROOT = Path(__file__).resolve().parents[2]
STEMS = ("maelstrom_input_command", "maelstrom_small_angle_command",
         "maelstrom_angle_boundary_command", "maelstrom_angle_quadrant_command")


class MaelstromInputTests(TestCase):
    def test_primary_factory_and_all_closed_retained_recipes(self):
        self.assertEqual(request(), json.loads((ROOT / "tools/rhino_oracle/fixtures/maelstrom_input_command.json").read_text()))
        count = 0
        for stem in STEMS:
            data = json.loads((ROOT / ("tools/rhino_oracle/fixtures/" + stem + ".json")).read_text())
            for op in data["operations"]:
                validate(op)
                count += 1
        self.assertEqual(count, 44)

    def test_rejects_unknown_fields_unbounded_and_injected_values(self):
        base = request()["operations"][0]
        for changes in (dict(id="x\n_Exit"), dict(extra=True), dict(normal=[0.,0.,0.]),
                        dict(radius0="_Exit"), dict(radius1=True), dict(center=[101.,0.,0.]),
                        dict(radius0=float("nan")), dict(angles=[float("inf")]),
                        dict(angles=[[0.,0.,101.]]), dict(angles=[True]), dict(angles=[]),
                        dict(angles=[0.] * 5, copy=True), dict(angles=[0.,1.], copy=False),
                        dict(angles=[None,0.], copy=True), dict(postselect=1), dict(copy="Yes")):
            op = copy.deepcopy(base)
            op.update(changes)
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                validate(op)

    def test_requires_private_scheme_and_single_iteration_before_launch(self):
        with mock.patch.dict(os.environ, {"DISPLAY": ":987", "VIBOCEROS_ORACLE_HEADLESS": ":987"}):
            for scheme, iterations in ((None, 1), ("VibocerosOracleInputTest", 2), ("VibocerosOracleInputTest", True)):
                client = OracleClient(launcher=Path(__file__), settings_scheme=scheme)
                data = request()
                data["iterations"] = iterations
                with mock.patch("subprocess.Popen", side_effect=AssertionError("unexpected launch")), self.assertRaises(OracleProtocolError):
                    client.run_rhino(data)

    def test_native_terminal_points_empty_history_and_quadrant_branch(self):
        count = 0
        for stem in STEMS:
            inputs = json.loads((ROOT / ("tools/rhino_oracle/fixtures/" + stem + ".json")).read_text())["operations"]
            result = json.loads((ROOT / ("tools/rhino_oracle/observations/" + stem + ".json")).read_text())
            self.assertEqual(result["engine_version"], "8.32.26160.13001")
            self.assertEqual(len(inputs), len(result["results"]))
            for op, row in zip(inputs, result["results"]):
                self.assertEqual(op["id"], row["id"])
                v = row["value"]
                events = [e for e in v["events"] if e["name"] == "Maelstrom"]
                self.assertEqual(len(events), 1)
                self.assertIn(events[0]["result"], ("Success", "Cancel"))
                self.assertEqual(v["success"], events[0]["result"] == "Success")
                if events[0]["result"] == "Cancel":
                    self.assertTrue(op["copy"])
                    self.assertEqual(len(v["after"]), 21)
                for phase in ("before", "after", "undo", "redo"):
                    self.assertTrue(all(math.isfinite(p) for obj in v[phase] for p in obj["point"]))
                if op["radius1"] is None or op["angles"] == [None]:
                    self.assertEqual(v["after"], v["before"])
                    self.assertEqual(v["undo"], [])
                    self.assertNotIn("Morphed", v["history"])
                else:
                    self.assertEqual(len(v["undo"]), 7)
                count += 1
        self.assertEqual(count, 44)
        data = json.loads((ROOT / "tools/rhino_oracle/observations/maelstrom_angle_quadrant_command.json").read_text())
        f = ((math.sqrt(5.) - 2.) / 3.) ** 2 * (3. - 2. * (math.sqrt(5.) - 2.) / 3.)
        for index in (1, 7):
            point = data["results"][index]["value"]["after"][-7]["point"]
            angle = math.degrees(math.atan2(2. * point[1] - point[0], 2. * point[0] + point[1]) / f)
            self.assertAlmostEqual(angle, -239.03624346792648, places=10)

    def test_provenance_hashes(self):
        data = json.loads((ROOT / "docs/maelstrom-input-provenance.json").read_text())
        self.assertTrue(data["private_xvfb"])
        self.assertEqual(data["native_input_recipes"], 44)
        for path, expected in data["sha256"].items():
            self.assertEqual(hashlib.sha256((ROOT / path).read_bytes()).hexdigest(), expected, path)
