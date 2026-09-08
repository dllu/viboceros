"""Fault injection for the standalone worker's actual lifecycle functions."""
import ast
import json
import os
import tempfile
import time
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import Mock


class IdleEvent:
    def __init__(self, fail=False):
        self.removals = 0
        self.fail = fail

    def __isub__(self, callback):
        self.removals += 1
        if self.fail:
            raise RuntimeError("detach failed")
        return self


class GroupPickingWorkerTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.event = IdleEvent()
        self.app = SimpleNamespace(Idle=self.event, Version="test", RunScript=Mock())
        self.document = SimpleNamespace(Objects=Mock(), Groups=Mock(), Layers=Mock())
        source = Path(__file__).with_name("group_picking_worker.py").read_text()
        tree = ast.parse(source)
        # Import no Rhino runtime and execute no startup/file/event side effects.
        # The functions under test are compiled unchanged from the shipped worker.
        definitions = ast.Module(body=[node for node in tree.body if isinstance(node, ast.FunctionDef)], type_ignores=[])
        self.state = dict(index=0, stage="setup", ids=[], groups=[], layers=[], results=[], busy=False, finished=False)
        self.worker = dict(root=str(self.root), os=os, json=json, time=time,
                           document=self.document, state=self.state, request=dict(operations=[]),
                           Rhino=SimpleNamespace(RhinoApp=self.app, Geometry=SimpleNamespace(Point3d=Mock())))
        exec(compile(definitions, "group_picking_worker.py", "exec"), self.worker)

    def response(self):
        return json.loads((self.root / "response.json").read_text())

    def test_finish_is_idempotent_and_exit_reentry_does_not_repeat_cleanup(self):
        self.state["ids"] = ["owned"]
        self.app.RunScript.side_effect = lambda *_: self.worker["on_idle"](None, None)
        self.worker["finish"]()
        self.worker["finish"]("late error")
        self.assertNotIn("error", self.response())
        self.assertEqual(self.event.removals, 1)
        self.document.Objects.Delete.assert_called_once_with("owned", True)
        self.app.RunScript.assert_called_once_with("_Exit _No", False)
        self.assertTrue(self.state["finished"])

    def test_cleanup_failures_do_not_skip_other_owned_resources_or_response(self):
        self.state.update(ids=["first", "second"], groups=[4], layers=[7])
        self.document.Layers = Mock()
        self.document.Objects.Unlock.side_effect = RuntimeError("unlock failed")
        self.document.Objects.Delete.side_effect = [RuntimeError("delete failed"), True]
        self.document.Groups.Delete.side_effect = RuntimeError("group failed")
        self.worker["finish"]("original failure")
        response = self.response()
        for message in ("original failure", "restore layer", "unlock failed", "delete failed", "group failed"):
            self.assertIn(message, response["error"])
        self.assertEqual(response["results"], [])
        self.assertEqual(self.document.Objects.Show.call_count, 2)
        self.assertEqual(self.document.Objects.Delete.call_count, 2)
        self.document.Layers.Delete.assert_called_once_with(7, True)
        self.assertEqual([self.state[key] for key in ("ids", "groups", "layers")], [[], [], []])
        self.worker["finish"]()
        self.assertEqual(self.document.Objects.Delete.call_count, 2)

    def test_callback_detach_failure_still_publishes_and_disables_work(self):
        self.event.fail = True
        self.worker["finish"]()
        self.worker["on_idle"](None, None)
        self.assertIn("detach failed", self.response()["error"])
        self.document.Objects.UnselectAll.assert_not_called()
        self.assertEqual(self.event.removals, 1)

    def test_false_delete_result_is_not_reported_as_success(self):
        self.state.update(ids=["owned"], groups=[4])
        self.document.Objects.Delete.return_value = False
        self.document.Groups.Delete.return_value = False
        self.worker["finish"]()
        self.assertIn("delete object: deletion returned false", self.response()["error"])
        self.assertIn("delete group: deletion returned false", self.response()["error"])
        self.document.Groups.Delete.assert_called_once_with(4)

    def test_setup_reentry_and_failure_are_bounded(self):
        self.worker["request"] = dict(operations=[dict(id="case", groups=[], seed=0)])
        self.document.Objects.UnselectAll.side_effect = lambda: self.worker["on_idle"](None, None)
        self.document.Objects.AddLine.side_effect = RuntimeError("setup failed")
        self.worker["on_idle"](None, None)
        self.document.Objects.UnselectAll.assert_called_once_with()
        self.document.Objects.AddLine.assert_called_once()
        self.assertIn("setup failed", self.response()["error"])
        self.assertFalse(self.state["busy"])
        self.assertTrue(self.state["finished"])

    def test_logging_and_exit_failures_do_not_corrupt_published_response(self):
        (self.root / "worker-progress.log").mkdir()
        self.app.RunScript.side_effect = RuntimeError("exit failed")
        self.worker["on_idle"](None, None)
        self.assertNotIn("error", self.response())
        self.assertFalse(self.state["busy"])
        self.assertTrue(self.state["finished"])

    def test_failed_membership_edit_disposes_attributes_and_cleans_inserted_objects(self):
        self.worker["request"] = dict(operations=[dict(id="case", groups=[[0, 1]], seed=0, reverse_bridge=True)])
        self.document.Objects.AddLine.side_effect = ["a", "b", "c"]
        self.document.Groups.Add.return_value = 4
        attributes = Mock()
        attributes.GetGroupList.return_value = [4]
        self.document.Objects.FindId.return_value.Attributes.Duplicate.return_value = attributes
        self.document.Objects.ModifyAttributes.side_effect = RuntimeError("attribute edit failed")
        self.worker["on_idle"](None, None)
        attributes.Dispose.assert_called_once_with()
        self.assertEqual(self.document.Objects.Delete.call_count, 3)
        self.document.Groups.Delete.assert_called_once_with(4)
        self.assertIn("attribute edit failed", self.response()["error"])

    def test_failed_layer_insertion_disposes_temporary_layer(self):
        self.worker["request"] = dict(operations=[dict(id="case", groups=[], seed=0, layer_mode="locked")])
        self.document.Objects.AddLine.side_effect = ["a", "b", "c"]
        layer = Mock()
        self.worker["Rhino"].DocObjects = SimpleNamespace(Layer=Mock(return_value=layer))
        self.document.Layers.Add.side_effect = RuntimeError("layer add failed")
        self.worker["on_idle"](None, None)
        layer.Dispose.assert_called_once_with()
        self.assertEqual(self.document.Objects.Delete.call_count, 3)
        self.document.Layers.Delete.assert_not_called()
        self.assertIn("layer add failed", self.response()["error"])

    def test_failed_pick_marker_aborts_instead_of_waiting_for_an_impossible_click(self):
        self.worker["request"] = dict(operations=[dict(id="case", groups=[], seed=0)])
        self.document.Objects.AddLine.side_effect = ["a", "b", "c"]
        point = SimpleNamespace(X=1, Y=2)
        view = SimpleNamespace(ActiveViewport=SimpleNamespace(WorldToClient=Mock(return_value=point)),
                               ClientToScreen=Mock(return_value=point))
        self.document.Views = SimpleNamespace(ActiveView=view, Redraw=Mock())
        self.worker["System"] = SimpleNamespace(Drawing=SimpleNamespace(Point=Mock()))
        (self.root / "worker-progress.log").mkdir()
        self.worker["on_idle"](None, None)
        self.assertIn("worker-progress.log", self.response()["error"])
        self.assertTrue(self.state["finished"])
        self.assertFalse(self.state["busy"])
        self.assertEqual(self.document.Objects.Delete.call_count, 3)
