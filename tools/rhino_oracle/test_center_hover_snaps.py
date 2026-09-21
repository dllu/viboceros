"""Integrity and prompt-time calibration of retained Center-hover observations."""
import hashlib
import json
import math
from pathlib import Path
import unittest

from . import split_edge_probe

ROOT = Path(__file__).resolve().parents[2]


class CenterHoverSnapTests(unittest.TestCase):
    def test_retained_record_hashes(self):
        provenance = json.loads((ROOT / "docs/center-hover-snaps-provenance.json").read_text())
        for path, digest in provenance["retained_file_sha256"].items():
            self.assertEqual(hashlib.sha256((ROOT / path).read_bytes()).hexdigest(), digest)

    def test_twenty_two_calibrated_clicks_preserve_history_and_original_hypotheses(self):
        request = json.loads((ROOT / "tools/rhino_oracle/fixtures/center_hover_snaps.json").read_text())
        observed = json.loads((ROOT / "tools/rhino_oracle/observations/center_hover_snaps.json").read_text())
        self.assertEqual(len(request["operations"]), 22)
        self.assertEqual(len(observed["results"]), 22)
        captures, misses, rejected_hypotheses = 0, 0, 0
        for operation, row in zip(request["operations"], observed["results"]):
            with self.subTest(id=operation["id"]):
                self.assertEqual(operation["id"], row["id"])
                operation["sources"][0]["brep"]["artifact_path"] = "/owned/box.3dm"
                split_edge_probe.validate(operation)
                value = row["value"]
                self.assertTrue(value["succeeded"] and value["history_tested"])
                self.assertEqual(value["before"], value["undo"])
                self.assertEqual(value["after"], value["redo"])
                self.assertEqual(value["before"][1:], value["after"][1:])
                self.assertEqual(value["command_history"].count("_Pause"), 2)
                before = value["before"][0]["geometry"]["brep"]["vertices"]
                after = value["after"][0]["geometry"]["brep"]["vertices"]
                self.assertEqual(after[:-1], before)
                pick = operation["inputs"][0]["pick"]
                self.assertTrue(operation["record_viewport"])
                self.assertEqual(len(value["pick_frames"]), 1)
                frame = value["pick_frames"][0]
                self.assertEqual(frame["aim"], pick["aim"])
                self.assertTrue(frame["perspective"])
                matrix = frame["world_to_screen"]
                self.assertEqual(len(matrix), 4)
                self.assertTrue(all(len(row) == 4 for row in matrix))
                self.assertTrue(all(math.isfinite(v) for row in matrix for v in row))
                self.assertTrue(all(math.isfinite(v) for v in frame["camera_location"] + frame["camera_direction"]))
                self.assertAlmostEqual(sum(v*v for v in frame["camera_direction"]), 1., places=12)
                world = frame["aim"] + [1.]
                h = [sum(a*b for a,b in zip(row,world)) for row in matrix]
                self.assertNotEqual(h[3], 0.)
                for i in range(2):
                    self.assertLess(abs(h[i]/h[3] - frame["aim_client"][i]), 1e-7)
                    click = frame["click_client"][i]
                    self.assertIs(type(click), int)
                    self.assertEqual(click, int(frame["aim_client"][i]) + pick["offset"][i])
                    self.assertGreaterEqual(click, 1)
                    self.assertLess(click, frame["size"][i] - 1)
                if operation["id"].endswith("empty-center"):
                    self.assertGreater(abs(after[-1][0] - 4.), 1.)
                    misses += 1
                else:
                    expected = 2. if operation["id"].endswith("near-end") else 6. if operation["id"].endswith("near-quad") else 4.
                    self.assertAlmostEqual(after[-1][0], expected, places=11)
                    self.assertEqual(after[-1][1:], [0.,0.])
                    captures += 1
                    if operation["id"] in ("circle-all-near-point", "circle-all-other-point"):
                        self.assertIn(pick["point"][0], (2.6,3.))
                        self.assertNotAlmostEqual(after[-1][0], pick["point"][0])
                        rejected_hypotheses += 1
        self.assertEqual((captures, misses, rejected_hypotheses), (14,8,2))
