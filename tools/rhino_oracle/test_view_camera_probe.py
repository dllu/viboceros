"""Whitelist and cleanup checks for the camera and CPlane View oracle."""
from copy import deepcopy
import json
from math import sqrt
from pathlib import Path
from types import SimpleNamespace
import unittest

from . import view_camera_probe


def fixture():
    return dict(
        op="view_camera_probe", id="camera", origin=[10, 20, 30],
        x_axis=[1, 1, 0], y_axis=[-1, 1, 1],
        projections=["Top", "Perspective"],
        directions=list(view_camera_probe.DIRECTIONS),
    )


class CameraProbeTests(unittest.TestCase):
    def test_recorded_cplane_view_uses_camera_target_and_right_up_axes(self):
        root = Path(__file__).parent
        request = json.loads((root / "fixtures/construction_plane_view.json").read_text())
        response = json.loads((root / "observations/construction_plane_view.json").read_text())
        for operation, record in zip(request["operations"], response["results"]):
            self.assertEqual(operation["id"], record["id"])
            rows = record["value"]
            report = view_camera_probe.compare_cplane_view(operation, rows)
            self.assertTrue(report["passed"])
            self.assertLess(max(row["cplane_error"] for row in report["views"]), 1e-14)
            changed = deepcopy(rows)
            changed[1]["cplane_origin"][0] += 0.01
            self.assertFalse(view_camera_probe.compare_cplane_view(operation, changed)["passed"])

    def test_recorded_plan_probe_checks_parallel_zoom_and_marks_perspective_limit(self):
        root = Path(__file__).parent
        operation = json.loads((root / "fixtures/view_camera_plan.json").read_text())["operations"][0]
        rows = json.loads((root / "observations/view_camera_plan.json").read_text())["results"][0]["value"]
        comparison = view_camera_probe.compare_to_viboceros(operation, rows)
        self.assertTrue(comparison["passed"])
        self.assertTrue(comparison["views"][0]["zoom_checked"])
        self.assertEqual(comparison["views"][0]["plan_parallel_scale_error"], 0.0)
        self.assertFalse(comparison["views"][1]["zoom_checked"])
        self.assertLess(comparison["views"][1]["plan_perspective_scale_ratio"], 1.0)
        self.assertLess(comparison["views"][1]["plan_perspective_ratio_error"], 1e-12)
        self.assertAlmostEqual(rows[1]["frustum_near_before"], rows[1]["frustum_near"])

    def test_comparison_checks_each_camera_and_cplane_property(self):
        operation = fixture()
        origin = operation["origin"]
        x_axis = [1 / sqrt(2), 1 / sqrt(2), 0]
        y_axis = [-1 / sqrt(3), 1 / sqrt(3), 1 / sqrt(3)]
        z_axis = view_camera_probe._cross(x_axis, y_axis)
        neg = lambda vector: [-value for value in vector]
        orientations = {
            "Top": (neg(z_axis), y_axis),
            "Bottom": (z_axis, neg(y_axis)),
            "Front": (y_axis, z_axis),
            "Back": (neg(y_axis), z_axis),
            "Right": (neg(x_axis), z_axis),
            "Left": (x_axis, z_axis),
        }
        rows = []
        for projection in operation["projections"]:
            for direction in operation["directions"]:
                forward, up = orientations[direction]
                rows.append(dict(
                    projection=projection, direction=direction,
                    perspective=projection == "Perspective",
                    cplane_origin=origin, cplane_x=x_axis, cplane_y=y_axis,
                    camera_direction=forward, camera_up=up,
                    camera_target=origin,
                    camera_location=[a - 50 * b for a, b in zip(origin, forward)],
                    camera_distance_before=50.0,
                    frustum_width_before=20.0,
                    frustum_width=20.0,
                ))
        self.assertTrue(view_camera_probe.compare_to_viboceros(operation, rows)["passed"])
        for field, value in [
            ("camera_up", [0, 0, 1]),
            ("cplane_x", [1, 0, 0]),
            ("camera_target", [0, 0, 0]),
            ("camera_location", [0, 0, 0]),
            ("camera_distance_before", 25.0),
            ("perspective", False),
        ]:
            with self.subTest(field=field):
                changed = deepcopy(rows)
                changed[-1][field] = value
                self.assertFalse(view_camera_probe.compare_to_viboceros(operation, changed)["passed"])
        changed = deepcopy(rows)
        changed[0]["frustum_width"] = 25.0
        self.assertFalse(view_camera_probe.compare_to_viboceros(operation, changed)["passed"])

    def test_whitelist_rejects_unbounded_or_malformed_input(self):
        view_camera_probe.validate(fixture())
        view_camera_probe.validate(dict(fixture(), directions=["Plan"]))
        view_camera_probe.validate(dict(fixture(), directions=["CPlaneView"]))
        for mutation in [
            dict(origin=[0, 0, float("nan")]),
            dict(origin=[True, 0, 0]),
            dict(camera_target=[0, float("inf"), 0]),
            dict(projections=["Top", "Top"]),
            dict(projections=["_Delete"]),
            dict(directions=["Top", "Top"]),
            dict(directions=["Plan", "Top"]),
            dict(directions=["Perspective"]),
        ]:
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                view_camera_probe.validate(dict(fixture(), **mutation))
        with self.assertRaises(ValueError):
            view_camera_probe.script("_Delete")
        self.assertEqual(view_camera_probe.script("Back"), "_SetView _CPlane _Back")
        self.assertEqual(view_camera_probe.script("Plan"), "_Plan")
        self.assertEqual(view_camera_probe.script("CPlaneView"), "_CPlane _View")

    def test_mouse_navigation_requires_a_bounded_two_point_view(self):
        operation = dict(fixture(), projections=["Perspective"],
                         directions=["WorldTwoPointPerspective"], mouse_drag=[60, -20])
        view_camera_probe.validate(operation)
        for mutation in [
            dict(mouse_drag=[101, 0]), dict(mouse_drag=[0, -101]),
            dict(mouse_drag=[True, 0]), dict(mouse_drag=[1.5, 0]),
            dict(mouse_drag=[0]), dict(mouse_drag="60,0"),
            dict(projections=["Top"]), dict(directions=["WorldPerspective"]),
            dict(directions=["WorldTwoPointPerspective", "Top"]),
        ]:
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                view_camera_probe.validate(dict(operation, **mutation))
        self.assertEqual(view_camera_probe.script("WorldPerspective"),
                         "_SetView _World _Perspective")
        self.assertEqual(view_camera_probe.script("WorldTwoPointPerspective"),
                         "_SetView _World _TwoPointPerspective")
        with self.assertRaisesRegex(ValueError, "native viewport tests"):
            view_camera_probe.compare_to_viboceros(operation, [])

    def test_probe_restores_projection_target_and_name_after_command_failure(self):
        class Vector:
            def __init__(self, x, y, z):
                self.X, self.Y, self.Z = x, y, z

        class Plane:
            def __init__(self, origin, x_axis, y_axis):
                self.Origin, self.XAxis, self.YAxis = origin, x_axis, y_axis
                self.IsValid = True

        class Viewport:
            Name = "original"
            IsPerspectiveProjection = False
            CameraLocation = Vector(0, 0, 10)
            CameraDirection = Vector(0, 0, -1)
            CameraUp = Vector(0, 1, 0)

            def __init__(self):
                self.projection = "original"
                self.CameraTarget = Vector(1, 2, 3)
                self.plane = Plane(Vector(0, 0, 0), Vector(1, 0, 0), Vector(0, 1, 0))

            def SetProjection(self, projection, name, _redraw):
                self.projection = projection
                self.IsPerspectiveProjection = projection == "Perspective"
                self.Name = name
                return True

            def SetConstructionPlane(self, plane):
                self.plane = plane
                return True

            def ConstructionPlane(self):
                return self.plane

            def SetViewProjection(self, original, _redraw):
                self.projection = original.projection
                self.IsPerspectiveProjection = False
                return True

            def SetCameraTarget(self, target, _redraw):
                self.CameraTarget = target
                return True

        viewport = Viewport()
        original_target = viewport.CameraTarget
        captured = []

        class Info:
            def __init__(self, view):
                self.projection = view.projection
                self.FrustumWidth = 20.0
                captured.append(self)

            def Dispose(self):
                self.disposed = True

        def run_script(_script, _echo):
            viewport.CameraTarget = Vector(99, 99, 99)
            return False

        host = dict(Rhino=SimpleNamespace(
            Geometry=SimpleNamespace(Point3d=Vector, Vector3d=Vector, Plane=Plane),
            Display=SimpleNamespace(DefinedViewportProjection=SimpleNamespace(
                Top="Top", Perspective="Perspective")),
            DocObjects=SimpleNamespace(ViewportInfo=Info),
            RhinoApp=SimpleNamespace(RunScript=run_script),
        ))
        with self.assertRaisesRegex(ValueError, "SetView CPlane command failed"):
            view_camera_probe.run(dict(fixture(), projections=["Top"], directions=["Top"]), viewport, host)
        self.assertEqual(viewport.projection, "original")
        self.assertIs(viewport.CameraTarget, original_target)
        self.assertEqual(viewport.Name, "original")
        self.assertTrue(captured[0].disposed)


if __name__ == "__main__":
    unittest.main()
