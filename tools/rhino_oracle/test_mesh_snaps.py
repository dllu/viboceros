"""Mesh snap observations, independent targets, settings grammar and restoration."""
import copy
import hashlib
import json
from pathlib import Path
from types import SimpleNamespace
import unittest

from . import mesh_snap_settings_probe as settings
from . import split_edge_probe
from .references import mesh_snaps, projected_near

ROOT = Path(__file__).resolve().parents[2]


class MeshSnapTests(unittest.TestCase):
    def test_historical_screen_discrepancies_and_corrected_mesh_targets_are_retained(self):
        request = json.loads((ROOT / "tools/rhino_oracle/fixtures/mesh_snaps.json").read_text())
        observed = json.loads((ROOT / "tools/rhino_oracle/observations/mesh_snaps.json").read_text())
        targets = json.loads((ROOT / "tools/rhino_oracle/fixtures/mesh_snap_targets.json").read_text())
        mesh_targets = json.loads((ROOT / "tools/rhino_oracle/fixtures/mesh_snap_weighted_targets.json").read_text())
        self.assertEqual(request, mesh_snaps.request())
        self.assertEqual(len(request["operations"]), 16)
        self.assertEqual(len(observed["results"]), 16)
        self.assertEqual(observed["engine_version"], "8.32.26160.13001")
        counts = dict(Mid=0, Near=0, misses=0, history=0)
        for item, row in zip(request["operations"], observed["results"]):
            with self.subTest(id=item["id"]):
                self.assertEqual(item["id"], row["id"])
                owned = copy.deepcopy(item)
                owned["sources"][0]["brep"]["artifact_path"] = "/owned/box.3dm"
                split_edge_probe.validate(owned)
                value = row["value"]
                frame = value["pick_frames"][0]
                for p, q in zip(projected_near.project(frame, frame["aim"]), frame["aim_client"]):
                    self.assertLess(abs(p-q), 1e-7)
                target = mesh_snaps.reference_target(item, frame)
                self.assertEqual(target, targets[item["id"]])
                mesh_target = mesh_snaps.reference_target(item,frame,mesh_weighted=True)
                self.assertEqual(mesh_target,mesh_targets[item["id"]])
                self.assertEqual(value["mesh_snap_setting"]["requested"], item["snap_to_meshes"])
                self.assertEqual(value["mesh_snap_setting"]["before"], value["mesh_snap_setting"]["restored"])
                self.assertEqual(value["before"][1:], value["after"][1:])
                if value["history_tested"]:
                    counts["history"] += 1
                    self.assertEqual(value["undo"], value["before"])
                    self.assertEqual(value["redo"], value["after"])
                if target is None:
                    counts["misses"] += 1
                    continue
                counts[target["kind"]] += 1
                self.assertTrue(value["succeeded"] and value["history_tested"])
                x = value["after"][0]["geometry"]["brep"]["vertices"][-1][0]
                self.assertLess(abs(x-mesh_target["point"][0]),1e-9)
                residual = abs(x-target["point"][0])
                if target["kind"] == "Mid":
                    self.assertLess(residual, 1e-9)
                else:
                    # The historical screen-nearest metric remains distinct;
                    # corrected mesh targets above now agree at the same epsilon.
                    self.assertGreater(residual, 1e-9)
                    self.assertLess(residual, 0.002)
        self.assertEqual(counts, dict(Mid=2, Near=5, misses=9, history=12))

    def test_recorded_command_switches_preserve_other_settings(self):
        request = json.loads((ROOT / "tools/rhino_oracle/fixtures/mesh_snap_switches.json").read_text())
        observed = json.loads((ROOT / "tools/rhino_oracle/observations/mesh_snap_switches.json").read_text())
        for operation, row in zip(request["operations"], observed["results"]):
            self.assertEqual(operation["id"], row["id"])
            states = row["value"]["states"]
            self.assertEqual(len(states), 7)
            initial = operation["snap_to_meshes"]
            self.assertEqual([s["snap_to_meshes"] for s in states], [initial,not initial,not initial,initial,initial,initial,not initial])
            base = {k:v for k,v in states[0].items() if k != "snap_to_meshes"}
            for state in states:
                self.assertEqual({k:v for k,v in state.items() if k != "snap_to_meshes"}, base)

    def test_probe_restores_mesh_setting_after_initialization_or_body_failure(self):
        class App:
            CommandHistoryWindowText = ""
            def __init__(self, fail=False): self.enabled, self.fail = False, fail
            def RunScript(self, command, echo):
                if command.endswith("_Enable"):
                    self.enabled = True
                    if self.fail: raise ValueError("set failed after mutation")
                elif command.endswith("_Disable"): self.enabled = False
                elif not command.endswith("_Cancel"): raise AssertionError(command)
                self.CommandHistoryWindowText += "Mesh object Near, Mid, Int, and Perp osnap support is " + ("enabled" if self.enabled else "disabled") + ".\n"
                return not command.endswith("_Cancel")
        for failure in (None, "init", "body"):
            app = App(failure == "init")
            host = dict(Rhino=SimpleNamespace(RhinoApp=app))
            def run():
                with settings.environment(True, host) as state:
                    self.assertTrue(app.enabled)
                    if failure == "body": raise ValueError("body failed")
                self.assertEqual(state, dict(before=False, requested=True, restored=False))
            if failure:
                with self.assertRaises(ValueError): run()
            else: run()
            self.assertFalse(app.enabled)
        for value in (None, 0, "Enable", [], {}):
            with self.assertRaises(ValueError), settings.environment(value, {}): pass

    def test_mesh_flag_validation_precedes_host_access(self):
        operation = mesh_snaps.request()["operations"][0]
        operation["sources"][0]["brep"]["artifact_path"] = "/owned/box.3dm"
        for value in (None, 0, "Enable", [], {}):
            with self.assertRaises(ValueError):
                split_edge_probe.run(dict(operation, snap_to_meshes=value), None, {})

    def test_retained_hashes(self):
        provenance = json.loads((ROOT / "docs/mesh-snaps-provenance.json").read_text())
        for path, digest in provenance["retained_file_sha256"].items():
            self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(),digest)
