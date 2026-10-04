"""Owned native Circle definitions, Maelstrom sizes and cancellation history."""
import copy
import hashlib
import json
import math
import os
from pathlib import Path
from unittest import TestCase, mock

from .client import OracleClient, OracleProtocolError
from .maelstrom_circle_probe import request, cross_request, memory_request, point_request, angle_request, validate

ROOT = Path(__file__).resolve().parents[2]
DATASETS = (("maelstrom_circle", request), ("maelstrom_circle_cross", cross_request),
            ("maelstrom_circle_memory", memory_request), ("maelstrom_circle_point", point_request), ("maelstrom_circle_angle", angle_request))


def read(folder, name):
    return json.loads((ROOT / "tools/rhino_oracle" / folder / (name + ".json")).read_text())


class MaelstromCircleTests(TestCase):
    def test_factories_closed_descriptors_and_retained_ids(self):
        for name, factory in DATASETS:
            fixture = read("fixtures", name)
            self.assertEqual(fixture, factory())
            result = read("observations", name)
            self.assertEqual(result["engine_version"], "8.32.26160.13001")
            self.assertEqual([op["id"] for op in fixture["operations"]], [row["id"] for row in result["results"]])
            for op in fixture["operations"]:
                validate(op)
        base = request()["operations"][0]
        for changes in (dict(id="x\n_Exit"), dict(extra=True), dict(normal=[0., 0., 0.]),
                        dict(inputs=["_Exit"]), dict(inputs=[True]), dict(inputs=[]),
                        dict(inputs=[[101., 0., 0.]]), dict(inputs=[float("nan")]),
                        dict(circle_inputs=[float("inf")]), dict(circle_inputs=False),
                        dict(target=True), dict(target=float("nan")), dict(target=101.),
                        dict(degrees="90 _Exit"), dict(degrees=True), dict(origin=[0., 0.])):
            op = copy.deepcopy(base)
            op.update(changes)
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                validate(op)

    def test_private_display_and_one_iteration_checked_before_launch(self):
        for scheme, display, headless, iterations in ((None, ":201", ":201", 1),
                ("VibocerosCircleTest", ":0", None, 1),
                ("VibocerosCircleTest", ":201", ":201", True),
                ("VibocerosCircleTest", ":201", ":201", 2)):
            data = request()
            data["iterations"] = iterations
            env = {"DISPLAY": display}
            if headless:
                env["VIBOCEROS_ORACLE_HEADLESS"] = headless
            with mock.patch.dict(os.environ, env, clear=True), mock.patch("tools.rhino_oracle.client._run_logged") as launch:
                with self.assertRaises((ValueError, OracleProtocolError)):
                    OracleClient(settings_scheme=scheme).run_rhino(data, 1)
                launch.assert_not_called()

    def test_native_circle_sdk_maps_command_points_and_history(self):
        compared = 0
        for name, _ in DATASETS:
            for row in read("observations", name)["results"]:
                v = row["value"]
                event = [e for e in v["events"] if e["name"] == "Maelstrom"]
                self.assertEqual(len(event), 1)
                self.assertEqual(v["success"], event[0]["result"] == "Success")
                for phase in ("before", "after", "undo", "redo"):
                    self.assertTrue(all(math.isfinite(c) for p in v[phase] for c in p["point"]))
                if not v["success"]:
                    self.assertEqual(v["before"], v["after"])
                    self.assertEqual(v["undo"], [])
                    self.assertEqual(v["redo"], v["before"])
                else:
                    self.assertEqual(v["undo"], v["before"])
                    self.assertTrue(all(not p["selected"] for p in v["redo"]))
                    self.assertEqual([p["point"] for p in v["redo"]], [p["point"] for p in v["after"]])
                # The cross-command probes intentionally construct a different Circle.
                if v["circle"] is None or name == "maelstrom_circle_cross":
                    continue
                self.assertEqual(len(v["sdk_points"]), 8)
                for actual, expected in zip(v["after"], v["sdk_points"]):
                    self.assertLessEqual(math.dist(actual["point"], expected), 1e-11, row["id"])
                c = v["circle"]
                if "seam" in c:
                    self.assertLessEqual(math.dist(c["seam"], [c["origin"][i] + c["radius"]*c["x"][i] for i in range(3)]), 1e-11)
                self.assertGreater(c["radius"], 0.)
                for axis in ("x", "y", "normal"):
                    self.assertAlmostEqual(sum(x*x for x in c[axis]), 1., places=13)
                compared += 1
        self.assertEqual(compared, 51)

    def test_radius_and_diameter_memory_is_independent_from_circle_command(self):
        cross = read("observations", "maelstrom_circle_cross")["results"]
        self.assertEqual(cross[0]["value"]["circle"]["radius"], 3.)
        self.assertIn("Diameter <8.000>", cross[1]["value"]["history"])
        self.assertEqual(cross[2]["value"]["circle"]["radius"], 5.)
        self.assertIn("Radius <4.000>", cross[3]["value"]["history"])
        memory = read("observations", "maelstrom_circle_memory")["results"]
        for i, radius in ((1, 4.), (3, 12./math.tau), (5, math.sqrt(16./math.pi))):
            self.assertAlmostEqual(memory[i]["value"]["circle"]["radius"], radius, places=13)
            self.assertTrue(memory[i]["value"]["diameter"])
            self.assertIn("Second diameter", memory[i]["value"]["history"])

    def test_rotated_cplane_circle_samples_are_retained(self):
        fixture = read("fixtures", "circle_two_point_rotated")
        result = read("observations", "circle_two_point_rotated")
        self.assertEqual(result["engine_version"], "8.32.26160.13001")
        self.assertEqual([op["id"] for op in fixture["operations"]], [row["id"] for row in result["results"]])
        for row in result["results"]:
            self.assertEqual(len(row["value"]["points"]), 17)
            self.assertTrue(all(math.isfinite(v) for p in row["value"]["points"] for v in p))
        self.assertEqual(result["results"][0]["value"]["points"][0], [2., 0., 0.])
        self.assertEqual(result["results"][2]["value"]["points"][0], [5., 4., 3.])

    def test_provenance_hashes_include_diagnostics(self):
        data = json.loads((ROOT / "docs/maelstrom-circle-provenance.json").read_text())
        self.assertTrue(data["private_xvfb"])
        self.assertEqual(data["native_circle_recipes"], 42)
        self.assertEqual(data["native_memory_recipes"], 6)
        self.assertEqual(data["native_cross_command_recipes"], 4)
        self.assertEqual(data["native_point_conversion_recipes"], 8)
        self.assertEqual(data["native_coordinate_angle_recipes"], 4)
        self.assertEqual(data["diagnostic_recipes"], 42)
        self.assertEqual(data["native_rotated_cplane_recipes"], 3)
        for path, expected in data["sha256"].items():
            self.assertEqual(hashlib.sha256((ROOT / path).read_bytes()).hexdigest(), expected, path)
