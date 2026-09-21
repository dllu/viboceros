"""Preserve raw circular-center evidence, including explicit disagreements."""
import hashlib
import json
import math
from pathlib import Path
import unittest

from . import split_edge_probe

ROOT = Path(__file__).resolve().parents[2]


class CircularCenterSnapTests(unittest.TestCase):
    def test_retained_hashes(self):
        provenance = json.loads((ROOT / "docs/circular-center-snaps-provenance.json").read_text())
        for path, digest in provenance["retained_file_sha256"].items():
            self.assertEqual(hashlib.sha256((ROOT / path).read_bytes()).hexdigest(), digest)

    def test_calibrated_history_and_explicit_capture_differences(self):
        request = json.loads((ROOT / "tools/rhino_oracle/fixtures/circular_center_snaps.json").read_text())
        observed = json.loads((ROOT / "tools/rhino_oracle/observations/circular_center_snaps.json").read_text())
        self.assertEqual(observed["engine_version"], "8.32.26160.13001")
        self.assertEqual(len(request["operations"]), 44)
        self.assertEqual(len(observed["results"]), 44)
        captures, misses, negative, elliptic, histories = 0, 0, 0, 0, 0
        for operation, row in zip(request["operations"], observed["results"]):
            with self.subTest(id=operation["id"]):
                self.assertEqual(operation["id"], row["id"])
                for source in operation["sources"]:
                    if "brep" in source:
                        source["brep"]["artifact_path"] = "/owned/source.3dm"
                split_edge_probe.validate(operation)
                value, pick = row["value"], operation["inputs"][0]["pick"]
                self.assertEqual(value["before"][1:], value["after"][1:])
                self.assertEqual(len(value["pick_frames"]), 1)
                frame = value["pick_frames"][0]
                self.assertEqual(frame["aim"], pick["aim"])
                matrix = frame["world_to_screen"]
                self.assertEqual(len(matrix), 4)
                self.assertTrue(all(len(r) == 4 for r in matrix))
                self.assertTrue(all(math.isfinite(v) for r in matrix for v in r))
                h = [sum(a*b for a, b in zip(r, frame["aim"] + [1.])) for r in matrix]
                for i in range(2):
                    self.assertLess(abs(h[i]/h[3] - frame["aim_client"][i]), 1e-7)
                    self.assertIs(type(frame["click_client"][i]), int)
                    self.assertEqual(frame["click_client"][i], int(frame["aim_client"][i]) + pick["offset"][i])
                if operation["id"].startswith("outside-arc-"):
                    self.assertFalse(value["succeeded"] or value["history_tested"])
                    self.assertEqual(value["before"], value["after"])
                    misses += 1
                    continue
                self.assertTrue(value["succeeded"] and value["history_tested"])
                self.assertEqual(value["before"], value["undo"])
                self.assertEqual(value["after"], value["redo"])
                histories += 1
                before = value["before"][0]["geometry"]["brep"]["vertices"]
                after = value["after"][0]["geometry"]["brep"]["vertices"]
                self.assertEqual(after[:-1], before)
                self.assertEqual(after[-1][1:], [0., 0.])
                if operation["id"].startswith(("quarter-negative-", "empty-center-")):
                    self.assertGreater(abs(after[-1][0] - 4.), 0.5)
                    if operation["id"].startswith("quarter-negative-"):
                        negative += 1
                    else:
                        misses += 1
                elif operation["id"].startswith(("ellipse-", "noncircle-")):
                    expected = 3.75 if operation["id"].startswith("noncircle-") else 4.
                    self.assertEqual(pick["point"][0], 4.)
                    self.assertAlmostEqual(after[-1][0], expected, places=9)
                    elliptic += 1
                else:
                    self.assertAlmostEqual(after[-1][0], 4., places=9)
                    captures += 1
        self.assertEqual((captures, misses, negative, elliptic, histories), (34, 4, 2, 4, 42))
