"""Retained polygon Center clicks, calibration, and original target hypotheses."""
import hashlib
import json
import math
from pathlib import Path
import unittest

from . import split_edge_probe

ROOT = Path(__file__).resolve().parents[2]


class PolygonCenterSnapTests(unittest.TestCase):
    def test_retained_record_hashes(self):
        provenance = json.loads((ROOT / "docs/polygon-center-snaps-provenance.json").read_text())
        for path, digest in provenance["retained_file_sha256"].items():
            self.assertEqual(hashlib.sha256((ROOT / path).read_bytes()).hexdigest(), digest)

    def test_forty_four_calibrated_clicks_history_and_unmodified_hypotheses(self):
        request = json.loads((ROOT / "tools/rhino_oracle/fixtures/polygon_center_snaps.json").read_text())
        observed = json.loads((ROOT / "tools/rhino_oracle/observations/polygon_center_snaps.json").read_text())
        self.assertEqual(observed["engine_version"], "8.32.26160.13001")
        self.assertEqual(len(request["operations"]), 44)
        self.assertEqual(len(observed["results"]), 44)
        captures, misses, rejected = 0, 0, 0
        for operation, row in zip(request["operations"], observed["results"]):
            with self.subTest(id=operation["id"]):
                self.assertEqual(operation["id"], row["id"])
                operation["sources"][0]["brep"]["artifact_path"] = "/owned/box.3dm"
                split_edge_probe.validate(operation)
                value, pick = row["value"], operation["inputs"][0]["pick"]
                self.assertTrue(value["succeeded"] and value["history_tested"])
                self.assertEqual(value["before"], value["undo"])
                self.assertEqual(value["after"], value["redo"])
                self.assertEqual(value["before"][1:], value["after"][1:])
                self.assertEqual(value["command_history"].count("_Pause"), 2)
                self.assertTrue(operation["record_viewport"])
                self.assertEqual(len(value["pick_frames"]), 1)
                frame = value["pick_frames"][0]
                self.assertEqual(frame["aim"], pick["aim"])
                matrix = frame["world_to_screen"]
                self.assertEqual(len(matrix), 4)
                self.assertTrue(all(len(row) == 4 for row in matrix))
                self.assertTrue(all(math.isfinite(v) for row in matrix for v in row))
                h = [sum(a*b for a, b in zip(row, frame["aim"] + [1.])) for row in matrix]
                for i in range(2):
                    self.assertLess(abs(h[i]/h[3] - frame["aim_client"][i]), 1e-7)
                    self.assertIs(type(frame["click_client"][i]), int)
                    self.assertEqual(frame["click_client"][i], int(frame["aim_client"][i]) + pick["offset"][i])
                before = value["before"][0]["geometry"]["brep"]["vertices"]
                after = value["after"][0]["geometry"]["brep"]["vertices"]
                self.assertEqual(after[:-1], before)
                if operation["id"].startswith(("open-", "near-closed-", "empty-center-", "surface-interior-", "warped-surface-")):
                    self.assertGreater(abs(after[-1][0] - pick["point"][0]), 0.5)
                    misses += 1
                else:
                    expected = pick["point"][0]
                    if operation["id"].startswith("subdivided-surface-"):
                        self.assertEqual(expected, 4.5)
                        expected = 4.75
                        rejected += 1
                    self.assertAlmostEqual(after[-1][0], expected, places=9)
                    self.assertEqual(after[-1][1:], [0., 0.])
                    captures += 1
        self.assertEqual((captures, misses, rejected), (34, 10, 2))
