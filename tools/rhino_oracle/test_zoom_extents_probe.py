"""Validate bounded live fitting captures and cleanup after command failure."""
import copy
import json
import os
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import patch

from . import zoom_extents_probe as probe
from .client import OracleClient, OracleProtocolError


def fixture():
    return json.loads((Path(__file__).parent / "fixtures/zoom_extents_camera.json").read_text())


class ZoomExtentsProbeTests(unittest.TestCase):
    def test_validation_rejects_unbounded_or_invalid_work(self):
        operation = fixture()["operations"][0]
        probe.validate(operation)
        for change in [dict(projection="Plan"), dict(method="Delete"), dict(min=[0, 0]),
                       dict(min=[float("nan"), 0, 0]), dict(max=[True, 0, 0]),
                       dict(min=[1e9, 0, 0]), dict(min=[100, 0, 0]), dict(border=0),
                       dict(border=True), dict(frustum_scale=float("inf")),
                       dict(vertical_shift=4), dict(border_command=1)]:
            case = dict(operation["cases"][0], **change)
            with self.subTest(change=change), self.assertRaises(ValueError):
                probe.validate(dict(operation, cases=[case]))
        for cases in [[], [{}] * 129, [None], "Extents"]:
            with self.subTest(cases=cases), self.assertRaises(ValueError):
                probe.validate(dict(operation, cases=cases))

    def test_client_rejects_repetition_before_launch(self):
        request = fixture()
        request["iterations"] = 2
        with patch.dict(os.environ, {"DISPLAY": ":101", "VIBOCEROS_ORACLE_HEADLESS": ":101"}), \
                patch("tools.rhino_oracle.client._run_logged") as launch, \
                self.assertRaisesRegex(OracleProtocolError, "one iteration"):
            OracleClient(launcher="/bin/true").run_rhino(request)
        launch.assert_not_called()

    def test_foreign_geometry_is_refused_before_camera_or_settings_mutations(self):
        Rhino = SimpleNamespace(RhinoDoc=SimpleNamespace(ActiveDoc=SimpleNamespace(Objects=[object()])))
        with self.assertRaisesRegex(ValueError, "empty document"):
            probe.run(fixture()["operations"][0], object(), dict(Rhino=Rhino))

    def test_failure_deletes_only_owned_points_and_restores_all_settings_independently(self):
        for failure in [None, 3]:
            with self.subTest(delete_failure=failure):
                settings = SimpleNamespace(DefinedViewSetCPlane=False, DefinedViewSetProjection=False,
                    ZoomExtentsParallelViewBorder=1.25, ZoomExtentsPerspectiveViewBorder=1.75)
                initial_settings = copy.copy(vars(settings))
                state = dict(added=[], deleted=[], target=None, plane=None, disposed=[])
                original_target, original_plane = object(), object()

                class Objects:
                    def __iter__(self):
                        return iter(())

                    def AddPoint(self, point):
                        result = len(state["added"]) + 1
                        state["added"].append(result)
                        return result

                    def Delete(self, object_id, quiet):
                        state["deleted"].append(object_id)
                        return object_id != failure

                class Info:
                    FrustumLeft, FrustumRight = -10., 10.
                    FrustumBottom, FrustumTop = -5., 5.
                    FrustumNear, FrustumFar = 1., 100.

                    def __init__(self, view):
                        pass

                    def SetFrustum(self, *args):
                        return True

                    def Dispose(self):
                        state["disposed"].append(self)

                viewport = SimpleNamespace(CameraTarget=original_target, Name="Original",
                    ConstructionPlane=lambda: original_plane,
                    SetProjection=lambda *args: True, SetViewProjection=lambda *args: True,
                    SetCameraTarget=lambda target, update: state.update(target=target),
                    SetConstructionPlane=lambda plane: state.update(plane=plane))
                Rhino = SimpleNamespace(
                    RhinoDoc=SimpleNamespace(ActiveDoc=SimpleNamespace(Objects=Objects())),
                    ApplicationSettings=SimpleNamespace(ViewSettings=settings),
                    DocObjects=SimpleNamespace(ViewportInfo=Info),
                    Geometry=SimpleNamespace(Point3d=lambda *args: args),
                    Display=SimpleNamespace(DefinedViewportProjection=SimpleNamespace(Top="Top")),
                    RhinoApp=SimpleNamespace(RunScript=lambda *args: False))
                operation = dict(fixture()["operations"][0], cases=[fixture()["operations"][0]["cases"][0]])
                with patch.object(probe, "snapshot", return_value={}):
                    with self.assertRaisesRegex(ValueError, "cleanup failed" if failure else "Zoom command failed"):
                        probe.run(operation, viewport, dict(Rhino=Rhino, empty_guid=0, progress=lambda text: None))
                self.assertEqual(state["deleted"], state["added"])
                self.assertEqual(vars(settings), initial_settings)
                self.assertIs(state["target"], original_target)
                self.assertIs(state["plane"], original_plane)
                self.assertEqual(viewport.Name, "Original")
                self.assertEqual(len(state["disposed"]), 2)

    def test_saved_capture_is_complete_and_matches_requested_cases(self):
        root = Path(__file__).parent
        request = fixture()
        capture = json.loads((root / "observations/zoom_extents_camera.json").read_text())
        rows = capture["results"][0]["value"]
        self.assertEqual(capture["engine"], "rhino")
        self.assertEqual(len(rows), 85)
        self.assertEqual([row["case"] for row in rows], request["operations"][0]["cases"])
        self.assertEqual({row["case"]["projection"] for row in rows}, set(probe.ZOOM_PROJECTIONS))
        self.assertEqual({row["case"].get("method", "Extents") for row in rows}, set(probe.METHODS))
        for row in rows:
            after = row["after"]
            self.assertEqual(len(after["projected_points"]), 8)
            self.assertEqual(after["perspective"], row["case"]["projection"] in probe.PROJECTIONS[1:])
            self.assertEqual(after["two_point_perspective"], row["case"]["projection"] == "TwoPointPerspective")
            self.assertAlmostEqual(after["frustum"][0], -after["frustum"][1])
            self.assertAlmostEqual(after["frustum"][2], -after["frustum"][3])

    def test_command_and_api_border_capture_retains_requested_values(self):
        root = Path(__file__).parent
        request = json.loads((root / "fixtures/zoom_extents_borders.json").read_text())
        capture = json.loads((root / "observations/zoom_extents_borders.json").read_text())
        probe.validate(request["operations"][0])
        rows = capture["results"][0]["value"]
        self.assertEqual(len(rows), 20)
        for row, case in zip(rows, request["operations"][0]["cases"]):
            self.assertEqual(row["case"], case)
            self.assertEqual(row["effective_border"], [case["border"], case["border"]])
            if case["border_command"]:
                self.assertIn("ParallelView", row["border_history"])
                self.assertIn("PerspectiveView", row["border_history"])


if __name__ == "__main__":
    unittest.main()
