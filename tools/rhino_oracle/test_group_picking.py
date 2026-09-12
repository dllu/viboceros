"""Idle input must target only a newly owned window, once per worker marker."""
import copy
import json
import tempfile
import subprocess
import unittest
from pathlib import Path
from unittest.mock import patch

from .client import OracleError, OracleProtocolError
from .group_picking import IdlePicker, validate_request


class GroupPickingTests(unittest.TestCase):
    def test_mesh_explode_observations_differ_from_split_in_retained_source_selection(self):
        root = Path(__file__).parent
        request = json.loads((root / "fixtures/mesh_explode_picking.json").read_text())
        observed = json.loads((root / "observations/mesh_explode_picking.json").read_text())
        split = json.loads((root / "observations/mesh_split_picking.json").read_text())
        validate_request(request)
        self.assertEqual(len(request["operations"]), 13)
        self.assertEqual(len(observed["results"]), 13)
        self.assertEqual(len(split["results"]), 13)
        for op, result, reference in zip(request["operations"], observed["results"], split["results"]):
            self.assertEqual(op["id"], result["id"])
            self.assertEqual(result["id"], reference["id"].replace("split-", "explode-", 1))
            value = result["value"]
            expected = copy.deepcopy(reference["value"])
            expected["explode_succeeded"] = expected.pop("split_succeeded")
            for output in expected["outputs"]:
                if output["original_identity"]:
                    output["selected"] = False
            self.assertEqual(value, expected)
        invalid = copy.deepcopy(request)
        invalid["operations"][0]["move"] = True
        with self.assertRaises(OracleProtocolError): validate_request(invalid)

    def test_mesh_split_observations_cover_identity_modes_and_untouched_source(self):
        root = Path(__file__).parent
        request = json.loads((root / "fixtures/mesh_split_picking.json").read_text())
        observed = json.loads((root / "observations/mesh_split_picking.json").read_text())
        self.assertEqual([op["id"] for op in request["operations"]],
                         [result["id"] for result in observed["results"]])
        for op, result in zip(request["operations"], observed["results"]):
            value = result["value"]
            self.assertTrue(value["split_succeeded"])
            memberships = [[i for i, members in enumerate(op["groups"]) if source in members] for source in range(3)]
            if op.get("reverse_bridge"): memberships[1].reverse()
            seed_groups = memberships[op["seed"]]
            selected = sorted(op["groups"][seed_groups[-1]]) if seed_groups else [op["seed"]]
            self.assertEqual(value["selected"], selected)
            for source in range(3):
                outputs = [obj for obj in value["outputs"] if obj["source"] == "source-%d" % source]
                restricted = source in op.get("hidden", []) or source in op.get("locked", [])
                retained = restricted or (source == 1 and op.get("layer_mode") == "locked")
                split = source in selected
                originals = [obj for obj in outputs if obj["original_identity"]]
                self.assertEqual(len(originals), int(retained or not split))
                self.assertEqual(len(outputs), 2 + int(retained) if split else 1)
                expected_mode = "Hidden" if source in op.get("hidden", []) else "Locked" if restricted else "Normal"
                for obj in outputs:
                    self.assertEqual(obj["mode"], expected_mode)
                    self.assertEqual(obj["selected"], split)
                    self.assertEqual(obj["groups"], memberships[source])
                    self.assertEqual(obj["layer_visible"], not (source == 1 and op.get("layer_mode") == "hidden"))
                    self.assertEqual(obj["layer_locked"], source == 1 and op.get("layer_mode") == "locked")
                    self.assertEqual(obj["faces"], 2 if obj["original_identity"] else 1)
                    self.assertEqual(len(obj["vertices"]), 3 * obj["faces"])

    def test_mesh_split_picking_request_rejects_incompatible_commands(self):
        request = json.loads(Path(__file__).with_name("fixtures").joinpath("mesh_split_picking.json").read_text())
        validate_request(request)
        for changes in [dict(move=True), dict(recall_previous=True), dict(recall_last=True),
                        dict(last_steps=[dict(kind="undo")]), dict(add_to_group_sources=[0])]:
            invalid = copy.deepcopy(request)
            invalid["operations"][0].update(changes)
            with self.subTest(changes=changes), self.assertRaises(OracleProtocolError):
                validate_request(invalid)

    def test_permanent_request_and_rejected_modes(self):
        request = json.loads(Path(__file__).with_name("fixtures").joinpath("group_picking.json").read_text())
        validate_request(request)
        for name in ("last_selection.json", "last_selection_history.json", "deletion_recall.json", "add_to_group_picking.json"):
            validate_request(json.loads(Path(__file__).with_name("fixtures").joinpath(name).read_text()))
        for changes in [dict(seed=True), dict(seed=3), dict(groups=[[0, 0]]),
                        dict(groups=[[False]]), dict(groups=[[3]]), dict(locked=[1, 1]),
                        dict(hidden=[-1]), dict(locked=[1], hidden=[1]),
                        dict(layer_mode="other"), dict(reverse_bridge=1), dict(move="Yes"),
                        dict(recall_previous="Yes"),
                        dict(recall_last="Yes"), dict(recall_last=True, move=False),
                        dict(recall_last=True, move=True, locked=[0]),
                        dict(last_steps=[dict(kind="recall")]),
                        dict(recall_last=True, move=True, last_steps=[dict(kind="select", objects=[4])]),
                        dict(recall_last=True, move=True, last_steps=[dict(kind="recall", deselect_others="Yes")]),
                        dict(id="bad\nPICK injected 0 0"), dict(op="group_memberships")]:
            invalid = copy.deepcopy(request)
            invalid["operations"][0].update(changes)
            with self.subTest(changes=changes), self.assertRaises(OracleProtocolError):
                validate_request(invalid)
        for invalid in [dict(request, iterations=True), dict(request, iterations=2),
                        dict(request, operations=[]), dict(request, protocol_version=2),
                        dict(request, operations=request["operations"] * 2)]:
            with self.assertRaises(OracleProtocolError): validate_request(invalid)

    def test_add_to_group_picking_rejects_unsafe_or_ambiguous_fixture_modes(self):
        request = json.loads(Path(__file__).with_name("fixtures").joinpath("add_to_group_picking.json").read_text())
        for changes in [dict(add_to_group_sources=[]), dict(add_to_group_sources=[True]),
                        dict(add_to_group_sources=[3]), dict(add_to_group_sources=[0,0]),
                        dict(groups=[]), dict(move=True), dict(hidden=[2]), dict(locked=[0]),
                        dict(layer_mode="locked"), dict(recall_previous=True)]:
            invalid = copy.deepcopy(request)
            invalid["operations"][0].update(changes)
            with self.subTest(changes=changes), self.assertRaises(OracleProtocolError): validate_request(invalid)

    def test_clicks_only_owned_window_once_and_never_sends_enter(self):
        with tempfile.TemporaryDirectory() as directory:
            job = Path(directory)
            (job / "worker-progress.log").write_text("PICK case-0 123 456\n")
            picker = IdlePicker()
            with patch("tools.rhino_oracle.group_picking.time.monotonic", return_value=1.0), patch(
                "tools.rhino_oracle.group_picking._rhino_window_for_pids", return_value=None
            ) as window, patch("tools.rhino_oracle.group_picking.subprocess.run") as run:
                picker(job, set())
                window.assert_not_called()
                run.assert_not_called()
            with patch("tools.rhino_oracle.group_picking.time.monotonic", return_value=2.0), patch(
                "tools.rhino_oracle.group_picking._rhino_window_for_pids", return_value=None
            ) as window, patch("tools.rhino_oracle.group_picking.subprocess.run") as run:
                picker(job, {17})
                window.assert_called_once_with({17})
                run.assert_not_called()
                self.assertFalse((job / "click-ack.json").exists())
            with patch("tools.rhino_oracle.group_picking.time.monotonic", return_value=3.0), patch(
                "tools.rhino_oracle.group_picking._rhino_window_for_pids", return_value="owned-window"
            ) as window, patch("tools.rhino_oracle.group_picking.subprocess.run") as run:
                picker(job, {17})
                picker(job, {17})
                run.assert_called_once_with(["xdotool", "windowactivate", "--sync", "owned-window",
                    "mousemove", "123", "456", "click", "1"], check=True, timeout=10)
                self.assertEqual(json.loads((job / "click-ack.json").read_text()), "case-0")
                self.assertFalse((job / "click-ack.json.tmp").exists())

    def test_failed_click_does_not_acknowledge_or_suppress_retry(self):
        with tempfile.TemporaryDirectory() as directory:
            job = Path(directory)
            (job / "worker-progress.log").write_text("PICK case 123 456\n")
            picker = IdlePicker()
            picker.ready["case"] = 0
            with patch("tools.rhino_oracle.group_picking._rhino_window_for_pids", return_value="owned"), patch(
                "tools.rhino_oracle.group_picking.subprocess.run", side_effect=RuntimeError("input failed")
            ), self.assertRaises(RuntimeError):
                picker(job, {17})
            self.assertFalse((job / "click-ack.json").exists())
            self.assertNotIn("case", picker.seen)

    def test_distinct_cases_at_the_same_coordinates_each_receive_one_click(self):
        with tempfile.TemporaryDirectory() as directory:
            job = Path(directory)
            picker = IdlePicker()
            picker.ready.update(first=0, second=0)
            with patch("tools.rhino_oracle.group_picking._rhino_window_for_pids", return_value="owned"), patch(
                "tools.rhino_oracle.group_picking.subprocess.run"
            ) as run:
                (job / "worker-progress.log").write_text("PICK first 123 456\n")
                picker(job, {17})
                (job / "worker-progress.log").write_text("PICK first 123 456\nPICK second 123 456\n")
                picker(job, {17})
                picker(job, {17})
                self.assertEqual(run.call_count, 2)
                self.assertEqual(run.call_args_list[0], run.call_args_list[1])
                self.assertEqual(json.loads((job / "click-ack.json").read_text()), "second")

    def test_input_timeout_reports_case_and_progress_without_acknowledging(self):
        with tempfile.TemporaryDirectory() as directory:
            job = Path(directory)
            (job / "worker-progress.log").write_text("worker: started\nPICK case 123 456\n")
            picker = IdlePicker()
            picker.ready["case"] = 0
            with patch("tools.rhino_oracle.group_picking._rhino_window_for_pids", return_value="owned"), patch(
                "tools.rhino_oracle.group_picking.subprocess.run", side_effect=subprocess.TimeoutExpired("xdotool", 10)
            ), self.assertRaises(OracleError) as caught:
                picker(job, {17})
            self.assertIn("case in owned window owned", str(caught.exception))
            self.assertIn("worker: started", str(caught.exception))
            self.assertFalse((job / "click-ack.json").exists())
            self.assertNotIn("case", picker.seen)
