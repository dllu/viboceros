"""Test the worker's host-independent orchestration with a simulated document."""

import importlib.util
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import patch


class RhinoWorkerTests(unittest.TestCase):
    def setUp(self):
        self.document = SimpleNamespace(
            ModelAbsoluteTolerance=0.01,
            ModelRelativeTolerance=0.01,
            ModelAngleToleranceRadians=0.02,
        )
        self.original = vars(self.document).copy()
        rhino = SimpleNamespace(
            RhinoDoc=SimpleNamespace(ActiveDoc=self.document),
            RhinoApp=SimpleNamespace(Version="test"),
        )
        spec = importlib.util.spec_from_file_location(
            "oracle_worker_test", Path(__file__).with_name("rhino_worker.py")
        )
        self.worker = importlib.util.module_from_spec(spec)
        with patch.dict("sys.modules", {"Rhino": rhino, "System": SimpleNamespace()}):
            spec.loader.exec_module(self.worker)
        self.request = {
            "protocol_version": 1,
            "operations": [{"id": "command", "op": "stub"}],
            "tolerance": {"absolute": 1e-8, "relative": 1e-10, "angular": 1e-6},
        }

    def test_applies_fixture_tolerance_during_commands_and_restores_document(self):
        def execute(operation, iterations, tolerance):
            self.assertEqual(self.document.ModelAbsoluteTolerance, 1e-8)
            self.assertEqual(self.document.ModelRelativeTolerance, 1e-10)
            self.assertEqual(self.document.ModelAngleToleranceRadians, 1e-6)
            return {"succeeded": True}, 100

        with patch.object(self.worker, "_execute", side_effect=execute), patch.object(
            self.worker, "_record_progress"
        ):
            response = self.worker._response(self.request)
        self.assertNotIn("error", response)
        self.assertEqual(response["results"][0]["value"], {"succeeded": True})
        self.assertEqual(vars(self.document), self.original)

    def test_failed_command_restores_document_and_discards_partial_results(self):
        self.request["operations"].append({"id": "failing", "op": "stub"})
        with patch.object(
            self.worker, "_execute", side_effect=[({"succeeded": True}, 100), ValueError("failed")]
        ), patch.object(self.worker, "_record_progress"):
            response = self.worker._response(self.request)
        self.assertIn("failed", response["error"])
        self.assertEqual(response["results"], [])
        self.assertEqual(vars(self.document), self.original)

    def test_rejected_tolerance_restores_settings_before_any_command_runs(self):
        class RejectingDocument(SimpleNamespace):
            def __setattr__(self, name, value):
                if name == "ModelRelativeTolerance" and value != 0.01:
                    return
                super().__setattr__(name, value)

        document = RejectingDocument(**self.original)
        self.worker.Rhino.RhinoDoc.ActiveDoc = document
        with patch.object(self.worker, "_execute") as execute:
            response = self.worker._response(self.request)
        execute.assert_not_called()
        self.assertIn("did not accept", response["error"])
        self.assertEqual(vars(document), self.original)


if __name__ == "__main__":
    unittest.main()
