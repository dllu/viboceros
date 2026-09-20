"""Fraction sampling owns and normalizes only its Rhino shape reference."""
from types import SimpleNamespace as NS
from unittest import TestCase
from unittest.mock import Mock, patch
from . import test_worker


class CurveParameterSampleTests(TestCase):
    def setUp(self):
        test_worker.RhinoWorkerTests.setUp(self)
        interval = lambda a, b: NS(T0=a, T1=b, ParameterAt=lambda t: a*(1-t)+b*t)
        self.source = NS(Domain=interval(1e12, 1e12+2), IsValid=True, Dispose=Mock(),
                         SpanCount=2, SpanDomain=lambda i: interval(i/2.0, (i+1)/2.0),
                         PointAt=Mock(side_effect=lambda t: NS(X=t, Y=t*t, Z=0)),
                         DerivativeAt=Mock(side_effect=lambda t, n, side: [NS(X=t, Y=t*t, Z=0)]))
        self.worker.Rhino.Geometry = NS(Interval=interval,
                                       CurveEvaluationSide=NS(Below="left", Above="right"))

    def execute(self, fractions=(0.0, 0.5, 1.0)):
        with patch.object(self.worker, "_nurbs_curve_from_definition", return_value=self.source), \
             patch.object(self.worker, "_measure", side_effect=lambda n, f: (f(), 42)):
            return self.worker._execute(dict(op="nurbs_curve_parameter_samples", curve={},
                                             fractions=fractions), 1, {})

    def test_shape_reference_normalization_and_endpoint_sides(self):
        value, elapsed = self.execute()
        self.assertEqual(value["domain"], [1e12, 1e12+2])
        self.assertEqual((self.source.Domain.T0, self.source.Domain.T1), (0, 1))
        self.assertEqual(value["points"], [[0,0,0], [0.5,0.25,0], [1,1,0]])
        self.assertEqual(value["span_points"][0], [[0,0,0], [0.25,0.0625,0], [0.5,0.25,0]])
        self.assertEqual(value["span_points"][1], [[0.5,0.25,0], [0.75,0.5625,0], [1,1,0]])
        self.assertEqual([c.args[2] for c in self.source.DerivativeAt.call_args_list],
                         ["right", "right", "left", "right", "right", "left"])
        self.assertTrue(all(c.args[1] == 0 for c in self.source.DerivativeAt.call_args_list))
        self.assertEqual(elapsed, 42)
        self.source.Dispose.assert_called_once_with()

    def test_invalid_fractions_rejected_before_allocating_reference(self):
        for fraction in [True, "0.5", -0.1, 1.1, float("nan"), float("inf")]:
            with patch.object(self.worker, "_nurbs_curve_from_definition") as build:
                with self.assertRaises(ValueError):
                    self.worker._curve_parameter_samples(dict(curve={}, fractions=[fraction]), 1)
                build.assert_not_called()

    def test_point_failure_disposes_reference(self):
        self.source.PointAt.side_effect = RuntimeError("point failure")
        with self.assertRaisesRegex(RuntimeError, "point failure"):
            self.execute()
        self.source.Dispose.assert_called_once_with()

    def test_missing_sided_result_disposes_reference(self):
        self.source.DerivativeAt.side_effect = None
        self.source.DerivativeAt.return_value = None
        with self.assertRaisesRegex(ValueError, "one-sided"):
            self.execute()
        self.source.Dispose.assert_called_once_with()

    def test_invalid_normalized_reference_is_not_sampled(self):
        self.source.IsValid = False
        with self.assertRaisesRegex(ValueError, "normalization"):
            self.execute()
        self.source.PointAt.assert_not_called()
        self.source.Dispose.assert_called_once_with()
