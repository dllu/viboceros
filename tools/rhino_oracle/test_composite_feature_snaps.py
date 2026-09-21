"""Retained composite/surface feature observations and honest Center diagnostics."""
import hashlib
import json
from pathlib import Path
import unittest

from . import split_edge_probe

ROOT = Path(__file__).resolve().parents[2]


class CompositeFeatureSnapTests(unittest.TestCase):
    def records(self, name):
        request = json.loads((ROOT / ("tools/rhino_oracle/fixtures/" + name + ".json")).read_text())
        response = json.loads((ROOT / ("tools/rhino_oracle/observations/" + name + ".json")).read_text())
        self.assertEqual(len(request["operations"]), len(response["results"]))
        rows = []
        for operation, result in zip(request["operations"], response["results"]):
            self.assertEqual(operation["id"], result["id"])
            operation["sources"][0]["brep"]["artifact_path"] = "/owned/box.3dm"
            split_edge_probe.validate(operation)
            value = result["value"]
            self.assertTrue(value["succeeded"] and value["history_tested"])
            self.assertEqual(value["before"], value["undo"])
            self.assertEqual(value["after"], value["redo"])
            self.assertEqual(value["before"][1:], value["after"][1:])
            self.assertEqual(value["command_history"].count("_Pause"), 2)
            vertices_before = value["before"][0]["geometry"]["brep"]["vertices"]
            vertices_after = value["after"][0]["geometry"]["brep"]["vertices"]
            self.assertEqual(vertices_after[:-1], vertices_before)
            rows.append((operation, vertices_after[-1]))
        return rows

    def test_retained_record_hashes(self):
        provenance = json.loads((ROOT / "docs/composite-feature-snaps-provenance.json").read_text())
        for path, digest in provenance["retained_file_sha256"].items():
            self.assertEqual(hashlib.sha256((ROOT / path).read_bytes()).hexdigest(), digest)

    def test_eleven_mid_end_captures_and_one_uncaptured_center_remain_distinct(self):
        rows = self.records("composite_feature_snaps")
        self.assertEqual(len(rows), 12)
        matched = 0
        for operation, point in rows:
            pick = operation["inputs"][0]["pick"]
            self.assertEqual(pick["offset"], [5, 0])
            if operation["id"] == "polycurve-arc-center":
                self.assertEqual(pick["osnap"], "Cen")
                self.assertAlmostEqual(point[0], 2.776693248934896, places=12)
                self.assertGreater(abs(point[0] - pick["point"][0]), 1.)
            else:
                self.assertIn(pick["osnap"], ("End", "Mid"))
                self.assertAlmostEqual(point[0], pick["point"][0], places=11)
                self.assertEqual(point[1:], [0., 0.])
                matched += 1
        self.assertEqual(matched, 11)

    def test_center_capture_depends_on_curve_hover_for_standalone_and_composite_arcs(self):
        rows = self.records("center_capture_diagnostics")
        self.assertEqual(len(rows), 5)
        self.assertEqual({op["id"] for op, _ in rows}, {
            "standalone-arc-center-offset", "standalone-arc-center-exact",
            "polycurve-arc-center-exact", "polycurve-arc-center-curve-hover",
            "standalone-arc-center-curve-hover"})
        for operation, point in rows:
            pick = operation["inputs"][0]["pick"]
            self.assertEqual(pick["osnap"], "Cen")
            if operation["id"].endswith("curve-hover"):
                self.assertEqual(pick["point"], [2.4, -2.8, 0])
                self.assertAlmostEqual(point[0], 4., places=11)
            else:
                self.assertEqual(pick["point"], [4, -4, 0])
                self.assertGreater(abs(point[0] - 4.), 1.)
