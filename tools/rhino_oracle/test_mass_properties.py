"""Mass measurements retain raw trim frames and dispose owned public objects."""
from types import SimpleNamespace
from unittest import TestCase
from unittest.mock import Mock, patch
from . import test_worker


class MassPropertyWorkerTests(TestCase):
    def setUp(self):
        test_worker.RhinoWorkerTests.setUp(self)

    def test_trim_records_are_untimed_and_geometry_mutation_is_rejected(self):
        for failure in [None, "area", "volume", "changed", "record"]:
            with self.subTest(failure=failure):
                trim = SimpleNamespace(Dispose=Mock())
                brep = SimpleNamespace(IsSolid=True, Faces=[SimpleNamespace(
                    Loops=[SimpleNamespace(Trims=[trim])])], Dispose=Mock())
                owned = []
                events = []
                def properties(kind):
                    events.append(kind)
                    if failure == kind:
                        return None
                    obj = SimpleNamespace(Area=2., Volume=3., Dispose=Mock())
                    owned.append(obj)
                    return obj
                def record(curve):
                    self.assertIs(curve, trim)
                    events.append("record")
                    if failure == "record":
                        raise ValueError("record failed")
                    return {"domain": [1e9, 1e9 + (8 if failure == "changed" and owned else 4)]}
                self.worker.Rhino.Geometry = SimpleNamespace(
                    AreaMassProperties=SimpleNamespace(Compute=lambda *args: properties("area")),
                    VolumeMassProperties=SimpleNamespace(Compute=lambda *args: properties("volume")))
                with patch.object(self.worker, "_trimmed_brep_from_definition", return_value=brep), \
                     patch.object(self.worker, "_nurbs_parameter_curve_definition", side_effect=record):
                    if failure:
                        message = {"area": "area integration failed", "volume": "volume integration failed",
                            "changed": "changed trim geometry", "record": "record failed"}[failure]
                        with self.assertRaisesRegex(ValueError, message):
                            self.worker._trimmed_surface_mass_properties({}, 2, {"relative": 1e-13, "absolute": 1e-12})
                    else:
                        value, elapsed = self.worker._trimmed_surface_mass_properties({}, 2, {"relative": 1e-13, "absolute": 1e-12})
                        self.assertEqual(value, {"area": 2., "volume": 3., "is_solid": True,
                            "trim_curves": [[[{"domain": [1e9, 1e9+4]}]]]})
                        self.assertGreaterEqual(elapsed, 0)
                        self.assertEqual(events, ["record"] + ["area", "volume"]*3 + ["record"])
                brep.Dispose.assert_called_once_with()
                trim.Dispose.assert_not_called()
                for obj in owned:
                    obj.Dispose.assert_called_once_with()
