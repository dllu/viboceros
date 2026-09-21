"""Bounded real snap picks and restoration of owned Rhino application settings."""
import copy
import hashlib
import json
from pathlib import Path
import unittest
from types import SimpleNamespace
from . import split_edge_probe


class SplitEdgeSnapProbeTests(unittest.TestCase):
    def operation(self, pick):
        return dict(op="split_edge_command", id="snap", edge=0, pick="mouse",
                    sources=[dict(brep=dict(artifact_path="/owned/source.3dm"))], inputs=[dict(pick=pick)])

    def test_pick_grammar_validates_before_rhino_access(self):
        valid = dict(point=[2,0,0], osnap="Point", offset=[5,0])
        invalid = [None, [], {}, dict(valid, arbitrary=True), dict(valid, osnap="Point _Delete"),
                   dict(valid, osnap=None), dict(valid, point=[2,0]), dict(valid, point=[True,0,0]),
                   dict(valid, point=[float("nan"),0,0]), dict(valid, point=[10**400,0,0]),
                   dict(valid, offset=[33,0]), dict(valid, offset=[True,0]),
                   dict(valid, offset=[0.5,0]), dict(valid, offset=[0]), dict(valid, offset=None)]
        for pick in invalid:
            with self.subTest(pick=pick), self.assertRaises(ValueError):
                split_edge_probe.run(self.operation(pick), None, {})
        for mode in ("NoSnap", "Point", "End", "Mid", "Cen", "Quad", "Near"):
            operation = self.operation(dict(valid, osnap=mode))
            self.assertEqual(split_edge_probe.mouse_command(operation, None, {}),
                             "_SplitEdge _Pause _%s _Pause _Enter" % mode)
        split_edge_probe.validate(self.operation(dict(point=[2,0,0], osnap="Point")))

    def test_snap_settings_restore_after_success_initialization_and_command_failure(self):
        class Settings:
            def __init__(self, values, fail=None):
                object.__setattr__(self, "values", values)
                object.__setattr__(self, "fail", fail)
            def __setattr__(self, key, value):
                # Catch accidental use of Rhino-9-only API properties in an 8 probe.
                if key not in self.values: raise AttributeError(key)
                if self.fail == key: raise ValueError("initialization failed")
                self.values[key] = value
            def __getattr__(self, key): return self.values[key]
            def GetCurrentState(self): return copy.deepcopy(self.values)
            def UpdateFromState(self, state):
                object.__setattr__(self, "values", copy.deepcopy(state))
                if self.fail == "restore": raise ValueError("restore failed")
        initial_aid = dict(GridSnap=True, Ortho=True, Planar=True, Osnap=False, OsnapModes=123,
                           OsnapPickboxRadius=9, ProjectSnapToCPlane=True, SnapToLocked=False,
                           SnapToOccluded=False, OnlySnapToSelected=True, Untouched="keep")
        initial_track = dict(UseSmartTrack=True, Untouched="keep")
        for failure in (None, "Ortho", "OsnapModes", "command", "restore"):
            aid = Settings(copy.deepcopy(initial_aid), failure)
            track = Settings(copy.deepcopy(initial_track))
            settings = SimpleNamespace(ModelAidSettings=aid, SmartTrackSettings=track,
                                       OsnapModes=SimpleNamespace(**{"None": 0, "Near": 2, "End": 131072}))
            host = dict(Rhino=SimpleNamespace(ApplicationSettings=settings))
            operation = self.operation(dict(point=[2,0,0], osnap="Near"))
            operation["persistent_snaps"] = ["Near", "End"]
            def exercise():
                with split_edge_probe.snapping_environment(operation, host):
                    self.assertFalse(aid.GridSnap or aid.Ortho or aid.Planar or aid.ProjectSnapToCPlane)
                    self.assertTrue(aid.Osnap and aid.SnapToLocked and aid.SnapToOccluded)
                    self.assertEqual(aid.OsnapModes, 2 | 131072)
                    self.assertFalse(aid.OnlySnapToSelected)
                    self.assertEqual(aid.OsnapPickboxRadius, 12)
                    self.assertFalse(track.UseSmartTrack)
                    if failure == "command": raise ValueError("command failed")
            if failure:
                with self.subTest(failure=failure), self.assertRaises(ValueError): exercise()
            else: exercise()
            self.assertEqual(aid.values, initial_aid)
            self.assertEqual(track.values, initial_track)

    def test_near_persistent_grammar_is_bounded_and_rejects_injected_modes(self):
        operation = self.operation(dict(point=[2,0,0], osnap="Persistent"))
        for modes in (["Near"], ["Point", "End", "Mid", "Cen", "Quad", "Near"]):
            operation["persistent_snaps"] = modes
            split_edge_probe.validate(operation)
            self.assertEqual(split_edge_probe.mouse_command(operation, None, {}),
                             "_SplitEdge _Pause _Pause _Enter")
        for modes in (["Near", "Near"], ["Near _Delete"], ["Persistent"], ["NoSnap"], None,
                      ["Point", "End", "Mid", "Cen", "Quad", "Near", "Point"]):
            operation["persistent_snaps"] = modes
            with self.subTest(modes=modes), self.assertRaises(ValueError):
                split_edge_probe.run(operation, None, {})

    def test_old_parameter_and_mouse_probes_do_not_touch_settings(self):
        for operation in (dict(parameters=[0.25]), dict(inputs=[dict(mouse=0.5)])):
            with split_edge_probe.snapping_environment(operation, {}): pass

    def test_retained_snaps_keep_full_history_targets_and_unsnapped_controls(self):
        root = Path(__file__).resolve().parents[2]
        metadata = json.loads((root / "docs/split-edge-snaps-provenance.json").read_text())
        for path,digest in metadata["retained_file_sha256"].items():
            self.assertEqual(hashlib.sha256((root/path).read_bytes()).hexdigest(),digest)
        request = json.loads((root / "tools/rhino_oracle/fixtures/split_edge_snaps_command.json").read_text())
        observed = json.loads((root / "tools/rhino_oracle/observations/split_edge_snaps_command.json").read_text())
        self.assertEqual(len(request["operations"]),17)
        self.assertEqual(len(observed["results"]),17)
        controls = []
        for operation,result in zip(request["operations"],observed["results"]):
            self.assertEqual(operation["id"],result["id"])
            operation["sources"][0]["brep"]["artifact_path"] = "/owned/source.3dm"
            split_edge_probe.validate(operation)
            value = result["value"]
            self.assertTrue(value["succeeded"] and value["history_tested"])
            self.assertEqual(value["undo"],value["before"])
            self.assertEqual(value["redo"],value["after"])
            self.assertEqual(value["before"][1:],value["after"][1:])  # All external targets stay intact.
            self.assertEqual(value["command_history"].count("_Pause"),1+sum("pick" in step or "mouse" in step for step in operation["inputs"]))
            if operation["id"].endswith("-nosnap"):
                controls.append(operation["id"])
                x = value["after"][0]["geometry"]["brep"]["vertices"][8][0]
                self.assertGreater(abs(x-2),0.1)
        self.assertEqual(controls,["point-on-edge-nosnap","point-off-edge-nosnap"])

    def test_nonuniform_midpoint_witnesses_distinguish_arc_length_from_parameter(self):
        root = Path(__file__).resolve().parents[2]
        rows = json.loads((root / "tools/rhino_oracle/observations/split_edge_snaps_command.json").read_text())["results"]
        for row in rows[-2:]:
            value = row["value"]
            if row["id"] == "nonuniform-nurbs-mid":
                definition = value["before"][1]["geometry"]["curve"]["definition"]
                parameter_midpoint = 4.
            else:
                self.assertEqual(row["id"],"nonuniform-brep-mid")
                definition = value["before"][0]["geometry"]["brep"]["edges"][0]["curve"]["definition"]
                parameter_midpoint = 3.5
            self.assertEqual(definition["degree"],2)
            controls = definition["control_points"]
            self.assertTrue(all(c["weight"] == 1 for c in controls))
            self.assertEqual(0.25*controls[0]["point"][0]+0.5*controls[1]["point"][0]+0.25*controls[2]["point"][0],parameter_midpoint)
            before = value["before"][0]["geometry"]["brep"]["vertices"]
            after = value["after"][0]["geometry"]["brep"]["vertices"]
            self.assertEqual(after[:len(before)],before)
            self.assertAlmostEqual(after[-1][0],5.,places=12)
