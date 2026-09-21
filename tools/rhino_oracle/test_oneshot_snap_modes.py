"""Retained, calibrated one-shot-to-persistent transitions in real SplitEdge."""
import hashlib
import json
from pathlib import Path
import unittest

from . import split_edge_probe

ROOT = Path(__file__).resolve().parents[2]


class OneShotSnapTests(unittest.TestCase):
    def test_retained_hashes(self):
        provenance = json.loads((ROOT / "docs/oneshot-snap-provenance.json").read_text())
        for path, digest in provenance["retained_file_sha256"].items():
            self.assertEqual(hashlib.sha256((ROOT / path).read_bytes()).hexdigest(), digest)

    def test_four_two_pick_transitions_preserve_geometry_history_and_each_calibration(self):
        request = json.loads((ROOT / "tools/rhino_oracle/fixtures/oneshot_snap_modes.json").read_text())
        observed = json.loads((ROOT / "tools/rhino_oracle/observations/oneshot_snap_modes.json").read_text())
        self.assertEqual(len(request["operations"]), 4)
        self.assertEqual(len(observed["results"]), 4)
        modes = set()
        for operation, row in zip(request["operations"], observed["results"]):
            with self.subTest(id=operation["id"]):
                self.assertEqual(operation["id"], row["id"])
                operation["sources"][0]["brep"]["artifact_path"] = "/owned/box.3dm"
                split_edge_probe.validate(operation)
                first, second = [step["pick"] for step in operation["inputs"]]
                modes.add(first["osnap"])
                self.assertEqual(second["osnap"], "Persistent")
                value = row["value"]
                self.assertTrue(value["succeeded"] and value["history_tested"])
                self.assertEqual(value["before"], value["undo"])
                self.assertEqual(value["after"], value["redo"])
                self.assertEqual(value["before"][1:], value["after"][1:])
                self.assertEqual(value["command_history"].count("_Pause"), 3)
                before = value["before"][0]["geometry"]["brep"]["vertices"]
                after = value["after"][0]["geometry"]["brep"]["vertices"]
                self.assertEqual(after[:-2], before)
                for vertex, target_x in zip(after[-2:], sorted([first["point"][0],second["point"][0]],reverse=True)):
                    self.assertAlmostEqual(vertex[0],target_x,places=11)
                    self.assertEqual(vertex[1:],[0.,0.])
                self.assertEqual(len(value["pick_frames"]), 2)
                for pick, frame in zip((first,second),value["pick_frames"]):
                    self.assertEqual(frame["aim"],pick["aim"])
                    h = [sum(a*b for a,b in zip(row,frame["aim"]+[1.])) for row in frame["world_to_screen"]]
                    self.assertNotEqual(h[3],0.)
                    for i in range(2):
                        self.assertLess(abs(h[i]/h[3]-frame["aim_client"][i]),1e-7)
                        self.assertEqual(frame["click_client"][i],int(frame["aim_client"][i])+pick["offset"][i])
                        self.assertGreaterEqual(frame["click_client"][i],1)
                        self.assertLess(frame["click_client"][i],frame["size"][i]-1)
        self.assertEqual(modes,{"Cen","End","Point","Mid"})
