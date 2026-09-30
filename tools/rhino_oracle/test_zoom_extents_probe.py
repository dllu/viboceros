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
                       dict(vertical_shift=4), dict(border_command=1), dict(clipping_probe=1),
                       dict(clip_constraints=[1,2]), dict(clip_constraints=[1,2,True,0,0]),
                       dict(clip_constraints=[1,2,0,float("nan"),0]), dict(clip_constraints=[1,1e13,0,0,0]),
                       dict(context_min=[0,0,0]), dict(context_min=[0,0,0],context_max=[-1,0,0]),
                       dict(context_min=[0,0,0],context_max=[1e9,0,0]), dict(context_hidden=1),
                       dict(display_mode="Rendered"), dict(depth_min=[0,0,0]),
                       dict(depth_query_before=True), dict(depth_query_before=1),
                       dict(redraw_sequence=[]), dict(redraw_sequence=[{}]),
                       dict(redraw_sequence=[{"redraw": True}] * 9),
                       dict(redraw_sequence=[{"zoom_factor": True}]),
                       dict(redraw_sequence=[{"camera_distance_scale": 0}]),
                       dict(redraw_sequence=[{"camera_pan": [0, float("inf")]}]),
                       dict(redraw_sequence=[{"camera_pan": [0, 1e7]}]),
                       dict(redraw_sequence=[{"visible": 1}]),
                       dict(redraw_sequence=[{"delete_geometry": False}]),
                       dict(redraw_sequence=[{"world_view": "Delete"}]),
                       dict(redraw_sequence=[{"command": "_Delete"}])]:
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
        for failure, hidden, late in [(None,False,False), (3,False,False),
                (None,True,False), (3,True,False), (None,False,True), (3,False,True)]:
            with self.subTest(delete_failure=failure, hidden=hidden, redraw_failure=late):
                settings = SimpleNamespace(DefinedViewSetCPlane=False, DefinedViewSetProjection=False,
                    ZoomExtentsParallelViewBorder=1.25, ZoomExtentsPerspectiveViewBorder=1.75)
                initial_settings = copy.copy(vars(settings))
                state = dict(added=[], deleted=[], target=None, plane=None, disposed=[], hidden=set(), shown=[])
                original_target, original_plane = object(), object()

                class Objects:
                    def __iter__(self):
                        return iter(())

                    def AddPoint(self, point, attributes=None):
                        result = len(state["added"]) + 1
                        state["added"].append(result)
                        if attributes is not None and not attributes.Visible:
                            state["hidden"].add(result)
                        return result

                    def Delete(self, object_id, quiet):
                        state["deleted"].append(object_id)
                        return object_id != failure and object_id not in state["hidden"]

                    def Show(self, object_id, ignore_layer_mode):
                        state["shown"].append(object_id)
                        state["hidden"].discard(object_id)
                        return True

                    def Hide(self, object_id, ignore_layer_mode):
                        state["hidden"].add(object_id)
                        return True

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

                viewport = SimpleNamespace(CameraTarget=original_target, Name="Original", DisplayMode="OriginalMode",
                    ConstructionPlane=lambda: original_plane,
                    SetProjection=lambda *args: True, SetViewProjection=lambda *args: True,
                    SetCameraTarget=lambda target, update: state.update(target=target),
                    SetConstructionPlane=lambda plane: state.update(plane=plane))
                def redraw():
                    raise RuntimeError("redraw failed")

                Rhino = SimpleNamespace(
                    RhinoDoc=SimpleNamespace(ActiveDoc=SimpleNamespace(Objects=Objects(),
                        Views=SimpleNamespace(Redraw=redraw))),
                    ApplicationSettings=SimpleNamespace(ViewSettings=settings),
                    DocObjects=SimpleNamespace(ViewportInfo=Info, ObjectAttributes=SimpleNamespace),
                    Geometry=SimpleNamespace(Point3d=lambda *args: args),
                    Display=SimpleNamespace(DefinedViewportProjection=SimpleNamespace(Top="Top"),
                        DisplayModeDescription=SimpleNamespace(FindByName=lambda name:name)),
                    RhinoApp=SimpleNamespace(RunScript=lambda *args: late))
                operation = dict(fixture()["operations"][0], cases=[fixture()["operations"][0]["cases"][0]])
                if hidden:
                    operation["cases"] = [dict(operation["cases"][0], context_min=[0,0,0],
                        context_max=[1,1,1], context_hidden=True, display_mode="Shaded")]
                if late:
                    operation["cases"] = [dict(operation["cases"][0],
                        redraw_sequence=[dict(visible=False)], display_mode="Ghosted")]
                with patch.object(probe, "snapshot", return_value={}):
                    with self.assertRaisesRegex(ValueError if failure or not late else RuntimeError,
                            "cleanup failed" if failure else "redraw failed" if late else "Zoom command failed"):
                        probe.run(operation, viewport, dict(Rhino=Rhino, empty_guid=0, progress=lambda text: None))
                self.assertEqual(state["deleted"], state["added"])
                self.assertEqual(vars(settings), initial_settings)
                self.assertIs(state["target"], original_target)
                self.assertIs(state["plane"], original_plane)
                self.assertEqual(viewport.Name, "Original")
                self.assertEqual(viewport.DisplayMode, "OriginalMode")
                self.assertEqual(len(state["disposed"]), 2)
                self.assertFalse(state["hidden"])
                self.assertEqual(state["shown"], state["added"] if hidden or late else [])

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

    def test_clipping_captures_cover_api_constraints_context_visibility_and_box_queries(self):
        root = Path(__file__).parent
        for name, count in [("viewport_clipping",81), ("viewport_clipping_context",72), ("viewport_box_depth",48)]:
            request = json.loads((root / ("fixtures/" + name + ".json")).read_text())
            capture = json.loads((root / ("observations/" + name + ".json")).read_text())
            probe.validate(request["operations"][0])
            rows = capture["results"][0]["value"]
            self.assertEqual(len(rows), count)
            self.assertEqual([row["case"] for row in rows], request["operations"][0]["cases"])
            if name == "viewport_clipping":
                self.assertEqual(sum("clip_constraints" in row["case"] for row in rows), 39)
                self.assertEqual(sum(row["clipping"].get("succeeded") is False for row in rows), 3)
                self.assertEqual(sum("after_redraw" in row["clipping"] for row in rows), 42)
            elif name == "viewport_clipping_context":
                self.assertEqual({row["case"]["display_mode"] for row in rows}, {"Wireframe","Shaded","Ghosted"})
                self.assertEqual(sum(row["case"]["context_hidden"] for row in rows), 36)
                self.assertTrue(all(row["case"]["method"] == "Selected" for row in rows))
            else:
                self.assertTrue(all(row["case"]["depth_query_before"] for row in rows))
                self.assertGreaterEqual(sum(not row["clipping"]["depth_query"]["intersects"] for row in rows), 6)

    def test_redraw_captures_match_all_requested_actions(self):
        root = Path(__file__).parent
        for name, count, steps in [("viewport_clipping_redraw",72,189),
                                  ("viewport_clipping_fallback",6,24)]:
            request = json.loads((root / ("fixtures/" + name + ".json")).read_text())
            capture = json.loads((root / ("observations/" + name + ".json")).read_text())
            probe.validate(request["operations"][0])
            rows = capture["results"][0]["value"]
            self.assertEqual(len(rows), count)
            self.assertEqual([row["case"] for row in rows], request["operations"][0]["cases"])
            self.assertEqual(sum(len(row["clipping"]["redraw_steps"]) for row in rows), steps)
            for row in rows:
                observed = row["clipping"]["redraw_steps"]
                self.assertEqual([step["action"] for step in observed], row["case"]["redraw_sequence"])
                for step in observed:
                    near, far = step["after"]["frustum"][4:]
                    self.assertGreater(near, 0)
                    self.assertGreater(far, near)


if __name__ == "__main__":
    unittest.main()
