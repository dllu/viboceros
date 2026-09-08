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
    def test_permanent_request_and_rejected_modes(self):
        request = json.loads(Path(__file__).with_name("fixtures").joinpath("group_picking.json").read_text())
        validate_request(request)
        for name in ("last_selection.json", "last_selection_history.json", "deletion_recall.json"):
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
