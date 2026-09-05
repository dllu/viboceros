"""Test the worker's host-independent orchestration with a simulated document."""

import importlib.util
import math
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch


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

    def test_differential_only_records_do_not_invoke_length_solvers(self):
        point = SimpleNamespace(X=0.0, Y=0.0, Z=0.0)
        tangent = SimpleNamespace(X=1.0, Y=0.0, Z=0.0)
        curve = SimpleNamespace(
            Domain=SimpleNamespace(T0=0.0, T1=1.0, ParameterAt=lambda t: t),
            IsClosed=False,
            PointAt=lambda t: point,
            DerivativeAt=lambda t, order: [point, tangent, point],
            TangentAt=lambda t: tangent,
            GetLength=Mock(return_value=1.0),
            NormalizedLengthParameter=Mock(side_effect=lambda t, tolerance: (True, t)),
        )
        with patch.object(self.worker, "_nurbs_curve_definition", return_value={}):
            differential = self.worker._curve_native_record(curve, {"relative": 1e-12}, True)
            self.assertEqual(len(differential["samples"]), 33)
            self.assertNotIn("length", differential)
            self.assertNotIn("divisions", differential)
            curve.GetLength.assert_not_called()
            curve.NormalizedLengthParameter.assert_not_called()

            complete = self.worker._curve_native_record(curve, {"relative": 1e-12})
            self.assertEqual(complete["length"], 1.0)
            self.assertEqual(len(complete["divisions"]), 18)
            curve.GetLength.assert_called_once_with(1e-12)
            self.assertEqual(curve.NormalizedLengthParameter.call_count, 16)

    def test_sided_records_use_derivative_points_and_restrict_stationary_tangents(self):
        class Vector:
            def __init__(self, source):
                self.X, self.Y, self.Z = source.X, source.Y, source.Z

            def Unitize(self):
                length = math.hypot(self.X, self.Y, self.Z)
                if length == 0:
                    return False
                self.X, self.Y, self.Z = self.X / length, self.Y / length, self.Z / length
                return True

        def xyz(x, y=0.0, z=0.0):
            return SimpleNamespace(X=x, Y=y, Z=z)

        self.worker.Rhino.Geometry = SimpleNamespace(
            CurveEvaluationSide=SimpleNamespace(Below=-1, Above=1), Vector3d=Vector,
            Interval=lambda a, b: SimpleNamespace(T0=a, T1=b),
        )
        for stationary in [False, True]:
            with self.subTest(stationary=stationary):
                pieces = []

                def trim(interval):
                    piece = SimpleNamespace(
                        TangentAt=Mock(return_value=Vector(xyz(-1.0 if interval.T1 == 1.0 else 1.0))),
                        Dispose=Mock(),
                    )
                    pieces.append(piece)
                    return piece

                curve = SimpleNamespace(
                    Domain=SimpleNamespace(T0=0.0, T1=2.0, ParameterAt=lambda t: 2.0 * t),
                    IsClosed=False, PointAt=lambda t: xyz(42.0), TangentAt=lambda t: xyz(1.0),
                    DerivativeAt=lambda t, order, side=0: [xyz(10.0 + side), xyz(0.0 if stationary else 1.0), xyz(0.0)],
                    Trim=Mock(side_effect=trim),
                )
                with patch.object(self.worker, "_nurbs_curve_definition", return_value={}):
                    value = self.worker._curve_native_record(curve, {"relative": 1e-12}, True, [1.0])
                self.assertEqual(value["sides"][0]["left"]["point"], [9.0, 0.0, 0.0])
                self.assertEqual(value["sides"][0]["right"]["point"], [11.0, 0.0, 0.0])
                if stationary:
                    intervals = [call.args[0] for call in curve.Trim.call_args_list]
                    self.assertEqual([(i.T0, i.T1) for i in intervals], [(0.0, 1.0), (1.0, 2.0)])
                    self.assertEqual(value["sides"][0]["left"]["tangent"], [-1.0, 0.0, 0.0])
                    self.assertEqual(value["sides"][0]["right"]["tangent"], [1.0, 0.0, 0.0])
                    for piece in pieces:
                        piece.TangentAt.assert_called_once_with(1.0)
                        piece.Dispose.assert_called_once_with()
                else:
                    curve.Trim.assert_not_called()


if __name__ == "__main__":
    unittest.main()
