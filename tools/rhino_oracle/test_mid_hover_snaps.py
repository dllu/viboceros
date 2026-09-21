"""Integrity and prompt-time calibration of retained Mid-only hover observations."""
import hashlib
import json
import math
from pathlib import Path
import unittest

from . import split_edge_probe

ROOT = Path(__file__).resolve().parents[2]


class MidHoverSnapTests(unittest.TestCase):
    def test_retained_record_hashes(self):
        provenance = json.loads((ROOT / "docs/mid-hover-snaps-provenance.json").read_text())
        for path, digest in provenance["retained_file_sha256"].items():
            self.assertEqual(hashlib.sha256((ROOT / path).read_bytes()).hexdigest(), digest)

    def test_thirty_seven_calibrated_clicks_and_mixed_mode_controls(self):
        request = json.loads((ROOT / "tools/rhino_oracle/fixtures/mid_hover_snaps.json").read_text())
        observed = json.loads((ROOT / "tools/rhino_oracle/observations/mid_hover_snaps.json").read_text())
        self.assertEqual(observed["engine_version"], "8.32.26160.13001")
        self.assertEqual(len(request["operations"]), 37)
        self.assertEqual(len(observed["results"]), 37)
        captures, misses, histories, failed_controls = 0, 0, 0, 0
        for operation, row in zip(request["operations"], observed["results"]):
            with self.subTest(id=operation["id"]):
                self.assertEqual(operation["id"], row["id"])
                operation["sources"][0]["brep"]["artifact_path"] = "/owned/box.3dm"
                split_edge_probe.validate(operation)
                value = row["value"]
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
                h = [sum(a*b for a, b in zip(row, world)) for row in matrix]
                self.assertNotEqual(h[3], 0.)
                for i in range(2):
                    self.assertLess(abs(h[i]/h[3] - frame["aim_client"][i]), 1e-7)
                    click = frame["click_client"][i]
                    self.assertIs(type(click), int)
                    self.assertEqual(click, int(frame["aim_client"][i]) + pick["offset"][i])
                    self.assertGreaterEqual(click, 1)
                    self.assertLess(click, frame["size"][i] - 1)
                self.assertEqual(value["before"][1:], value["after"][1:])
                mixed = operation["id"].endswith(("-mixed", "-mixed-hover"))
                if mixed:
                    self.assertEqual(operation["persistent_snaps"], ["Point", "Mid"])
                    self.assertEqual(pick["osnap"], "Persistent")
                    misses += 1
                if operation["id"] == "rational-mixed":
                    # The unsnapped screen-to-edge control fails; no Undo/Redo was run.
                    self.assertFalse(value["succeeded"])
                    self.assertFalse(value["history_tested"])
                    self.assertIsNone(value.get("undo"))
                    self.assertIsNone(value.get("redo"))
                    self.assertEqual(value["before"][0]["geometry"], value["after"][0]["geometry"])
                    failed_controls += 1
                    continue
                self.assertTrue(value["succeeded"] and value["history_tested"])
                self.assertEqual(value["before"], value["undo"])
                self.assertEqual(value["after"], value["redo"])
                self.assertEqual(value["command_history"].count("_Pause"), 2)
                before = value["before"][0]["geometry"]["brep"]["vertices"]
                after = value["after"][0]["geometry"]["brep"]["vertices"]
                self.assertEqual(after[:-1], before)
                histories += 1
                if mixed:
                    self.assertGreater(abs(after[-1][0] - pick["point"][0]), 0.5)
                else:
                    self.assertAlmostEqual(after[-1][0], pick["point"][0], places=9)
                    self.assertEqual(after[-1][1:], [0., 0.])
                    captures += 1
        self.assertEqual((captures, misses, histories, failed_controls), (26, 11, 36, 1))
