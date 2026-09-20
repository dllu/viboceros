"""Trim-aware isocurve direction, normalization, and owned-object cleanup."""
from types import SimpleNamespace
from unittest import TestCase
from unittest.mock import Mock, patch

from . import test_worker


def point(x, y, z):
    return SimpleNamespace(X=x, Y=y, Z=z)


def interval(a, b):
    return SimpleNamespace(T0=a, T1=b)


class IsocurveWorkerTests(TestCase):
    def setUp(self):
        test_worker.RhinoWorkerTests.setUp(self)
        self.worker.Rhino.Geometry = SimpleNamespace(Interval=interval)
        self.tolerance = dict(absolute=1e-9, relative=1e-12, angular=1e-10)
        self.operation = dict(op="trimmed_surface_isocurves", parameters=[[0.125, 0.375]])

    def run_probe(self, brep):
        with patch.object(self.worker, "_trimmed_brep_from_definition", return_value=brep), \
             patch.object(self.worker.Rhino.Geometry, "Interval", side_effect=interval, create=True), \
             patch.object(self.worker, "_measure", side_effect=lambda count, compute: (compute(), 0)):
            return self.worker._execute(self.operation, 1, self.tolerance)[0]

    def test_directions_and_nonsymmetric_stations_use_normalized_output_domains(self):
        def curve(start, end):
            return SimpleNamespace(Domain=interval(1e12, 1e12 + 1), Dispose=Mock(),
                                   PointAt=lambda t: point(start + (end-start)*t, 0.375, 0))
        reversed_curve, forward_curve = curve(3, 2), curve(0, 1)
        face = SimpleNamespace(Domain=Mock(return_value=interval(0, 1)),
                               TrimAwareIsoCurve=Mock(side_effect=[[reversed_curve, forward_curve], []]))
        brep = SimpleNamespace(Faces=[face], Dispose=Mock())
        result = self.run_probe(brep)
        self.assertEqual(face.Domain.call_args_list[0].args, (1,))
        self.assertEqual(face.Domain.call_args_list[1].args, (0,))
        self.assertEqual(face.TrimAwareIsoCurve.call_args_list[0].args, (0, 0.375))
        self.assertEqual(face.TrimAwareIsoCurve.call_args_list[1].args, (1, 0.125))
        self.assertEqual(result[0][0][1], [])
        for record, start in zip(result[0][0][0], [0, 2]):
            self.assertEqual(record, [[start+t, 0.375, 0] for t in [0, 0.125, 0.3, 0.5, 0.875, 1]])
        for owned in [reversed_curve, forward_curve, brep]:
            owned.Dispose.assert_called_once_with()

    def test_extracted_arrays_and_brep_are_disposed_on_sampling_failure(self):
        for failure in [RuntimeError("sampling"), float("nan"), float("inf")]:
            with self.subTest(failure=failure):
                first = SimpleNamespace(Domain=interval(0, 1), Dispose=Mock(),
                                        PointAt=Mock(side_effect=failure) if isinstance(failure, Exception)
                                        else Mock(return_value=point(failure, 0, 0)))
                second = SimpleNamespace(Dispose=Mock())
                face = SimpleNamespace(Domain=lambda axis: interval(0, 1),
                                       TrimAwareIsoCurve=lambda axis, fixed: [first, second])
                brep = SimpleNamespace(Faces=[face], Dispose=Mock())
                with self.assertRaises((RuntimeError, ValueError)):
                    self.run_probe(brep)
                for owned in [first, second, brep]:
                    owned.Dispose.assert_called_once_with()

    def test_invalid_queries_fail_without_leaking_or_constructing_unneeded_geometry(self):
        for parameters in [[], [[0, 0]] * 65, [[0]], [[0, 0, 0]], [[float("nan"), 0]]]:
            with self.subTest(parameters=parameters):
                self.operation["parameters"] = parameters
                with patch.object(self.worker, "_trimmed_brep_from_definition") as build:
                    with self.assertRaises(ValueError):
                        self.worker._execute(self.operation, 1, self.tolerance)
                    build.assert_not_called()
        self.operation["parameters"] = [[0.125, 0.375]]
        for end, curves in [(0.25, []), (1, None)]:
            face = SimpleNamespace(Domain=lambda axis: interval(0, end),
                                   TrimAwareIsoCurve=Mock(return_value=curves))
            brep = SimpleNamespace(Faces=[face], Dispose=Mock())
            with self.assertRaises(ValueError):
                self.run_probe(brep)
            brep.Dispose.assert_called_once_with()
            if end == 0.25:
                face.TrimAwareIsoCurve.assert_not_called()
