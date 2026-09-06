"""Signed surface inputs, public bounds queries, and single-face array records."""
from types import SimpleNamespace
from unittest import TestCase
from unittest.mock import Mock, patch
from . import test_worker


class SurfaceBoundsWorkerTests(TestCase):
    def setUp(self):
        test_worker.RhinoWorkerTests.setUp(self)
        progress = patch.object(self.worker, "_record_progress")
        progress.start()
        self.addCleanup(progress.stop)

    def test_signed_surface_controls_are_forwarded_in_u_fast_order(self):
        table = SimpleNamespace(SetPoint=Mock(return_value=True))
        surface = SimpleNamespace(Points=table)
        with patch.object(self.worker, "_point", side_effect=lambda p: p):
            controls = [{"point": [i, 2, 3], "weight": w} for i, w in enumerate([1, -0.1, -1e200, 1e-200])]
            self.worker._set_surface_controls(surface, controls, 2, 2)
            for i, c in enumerate(controls):
                self.assertEqual(table.SetPoint.call_args_list[i].args, (i % 2, i // 2, c["point"], c["weight"]))
            for bad in [0, float("nan"), float("inf")]:
                with self.assertRaises(ValueError):
                    self.worker._set_surface_controls(surface, [{"point": [0, 0, 0], "weight": bad}], 1, 1)

    def test_surface_bounds_use_the_accurate_api_and_dispose_on_success_or_failure(self):
        for valid in [True, False]:
            bounds = SimpleNamespace(IsValid=valid, Min=[1, 2, 3], Max=[4, 5, 6])
            surface = SimpleNamespace(IsValid=True, GetBoundingBox=Mock(return_value=bounds), Dispose=Mock())
            with patch.object(self.worker, "_nurbs_surface_from_definition", return_value=surface), \
                 patch.object(self.worker, "_xyz", side_effect=lambda p: p):
                if valid:
                    value, elapsed = self.worker._execute(dict(op="surface_bounds", surface={}), 2, {})
                    self.assertEqual(value, {"min": [1, 2, 3], "max": [4, 5, 6]})
                    self.assertGreaterEqual(elapsed, 0)
                else:
                    with self.assertRaises(ValueError):
                        self.worker._execute(dict(op="surface_bounds", surface={}), 2, {})
            self.assertTrue(surface.GetBoundingBox.called)
            self.assertTrue(all(call.args == (True,) for call in surface.GetBoundingBox.call_args_list))
            surface.Dispose.assert_called_once_with()

    def test_array_surface_records_unwrap_one_face_and_retain_both_native_domains(self):
        domains = [SimpleNamespace(T0=2, T1=6, ParameterAt=lambda t: 2 + 4*t),
                   SimpleNamespace(T0=-3, T1=5, ParameterAt=lambda t: -3 + 8*t)]
        surface = SimpleNamespace(Domain=lambda axis: domains[axis], PointAt=lambda u, v: [u, v, u*v])
        class Faces(list):
            @property
            def Count(self): return len(self)
        class Brep:
            def __init__(self, count): self.Faces = Faces([SimpleNamespace(UnderlyingSurface=lambda: surface)] * count)
        self.worker.Rhino.Geometry = SimpleNamespace(Brep=Brep)
        with patch.object(self.worker, "_xyz", side_effect=lambda p: p):
            expected = self.worker._plane_array_geometry_record(surface, True)
            actual = self.worker._plane_array_geometry_record(Brep(1), True)
            self.assertEqual(actual, expected)
            self.assertEqual(actual[0], [[2., 6.], [-3., 5.]])
            self.assertEqual(len(actual[1]), 25)
            self.assertEqual(actual[1][0], [2, -3, -6])
            self.assertEqual(actual[1][-1], [6, 5, 30])
            with self.assertRaisesRegex(ValueError, "single face"):
                self.worker._plane_array_geometry_record(Brep(2), True)

    def test_surface_diagnostic_samples_are_separate_from_the_reported_box(self):
        domains = [SimpleNamespace(ParameterAt=lambda t: 2 + 4*t),
                   SimpleNamespace(ParameterAt=lambda t: -3 + 8*t)]
        bounds = SimpleNamespace(IsValid=True, Min=[2, -3, -1], Max=[6, 5, 1])
        surface = SimpleNamespace(IsValid=True, Domain=lambda axis: domains[axis],
                                  PointAt=Mock(side_effect=lambda u, v: [u, v, u*v]),
                                  GetBoundingBox=Mock(return_value=bounds), Dispose=Mock())
        with patch.object(self.worker, "_nurbs_surface_from_definition", return_value=surface), \
             patch.object(self.worker, "_xyz", side_effect=lambda p: p):
            value, _ = self.worker._execute(dict(op="surface_bounds", surface={}, sample_grid=True), 2, {})
        self.assertEqual(value["min"], [2, -3, -1])
        self.assertEqual(value["max"], [6, 5, 1])
        self.assertEqual(value["sample_bounds"], {"min": [2, -3, -18], "max": [6, 5, 30]})
        self.assertEqual(surface.PointAt.call_count, 41*41)
        surface.Dispose.assert_called_once_with()
