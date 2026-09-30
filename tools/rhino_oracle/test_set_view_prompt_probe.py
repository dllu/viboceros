"""Bounded prompt macros and restoration checks, without launching Rhino."""
from copy import deepcopy
import json
from pathlib import Path
from types import SimpleNamespace
import unittest

from . import set_view_prompt_probe as probe


def fixture():
    return dict(op="set_view_prompt_probe", origin=[7, 8, 9],
                x_axis=[1, 1, 0], y_axis=[-1, 1, 1],
                camera_target=[21, -13, 17], steps=[["World", "Top"]])


class SetViewPromptProbeTests(unittest.TestCase):
    def test_macros_are_bounded_and_cannot_execute_arbitrary_commands(self):
        operation = fixture()
        probe.validate(operation)
        self.assertEqual(probe.script(["World", "Top"]), "_SetView _World _Top !")
        self.assertEqual(probe.script(["CPlane", ""]), "_SetView _CPlane _Enter !")
        for steps in [[], [["Delete"]], [["Top\n_Delete"]], [["_Enter"]],
                      [["Top"]] * 65, [["World"] * 9], ["World"], [[None]]]:
            invalid = deepcopy(operation)
            invalid["steps"] = steps
            with self.subTest(steps=steps), self.assertRaises(ValueError):
                probe.validate(invalid)
        for tokens in [[], ["Delete"], ["Top ! _Delete"], ["World"] * 9, "World"]:
            with self.subTest(tokens=tokens), self.assertRaises(ValueError):
                probe.script(tokens)

    def test_invalid_operation_and_nonfinite_camera_data_are_rejected(self):
        for field, value in [("op", "unknown"), ("camera_target", [0, 0, float("inf")]),
                             ("origin", [float("nan"), 0, 0]), ("x_axis", [1, 2]),
                             ("y_axis", None)]:
            operation = fixture()
            operation[field] = value
            with self.subTest(field=field), self.assertRaises(ValueError):
                probe.validate(operation)

    def test_failed_initialization_restores_all_view_fields_and_disposes_info(self):
        for restore_failure in [False, True]:
            calls = []
            target = object()
            plane = object()

            class Viewport:
                CameraTarget = target
                Name = "Original view"

                def ConstructionPlane(self):
                    return plane

                def SetConstructionPlane(self, value):
                    calls.append("plane")
                    self.plane = value
                    return True

                def SetProjection(self, *_):
                    self.Name = "Temporary probe name"
                    self.CameraTarget = object()
                    return False

                def SetViewProjection(self, *_):
                    calls.append("projection")
                    if restore_failure:
                        raise RuntimeError("projection restoration failed")
                    return True

                def SetCameraTarget(self, value, *_):
                    calls.append("target")
                    self.CameraTarget = value
                    return True

            class Info:
                def __init__(self, _):
                    pass

                def Dispose(self):
                    calls.append("dispose")

            viewport = Viewport()
            host = dict(Rhino=SimpleNamespace(
                DocObjects=SimpleNamespace(ViewportInfo=Info),
                Display=SimpleNamespace(DefinedViewportProjection=SimpleNamespace(
                    Perspective="Perspective"))))
            message = "cleanup failed: projection" if restore_failure else "could not initialize"
            with self.subTest(restore_failure=restore_failure), self.assertRaisesRegex(ValueError, message):
                probe.run(fixture(), viewport, host)
            self.assertEqual(calls, ["projection", "target", "plane", "dispose"])
            self.assertIs(viewport.CameraTarget, target)
            self.assertIs(viewport.plane, plane)
            self.assertEqual(viewport.Name, "Original view")

    def test_saved_capture_contains_each_requested_transition_in_order(self):
        root = Path(__file__).parent
        checked = 0
        for name in ["set_view_prompt", "set_view_nested_cplane"]:
            request = json.loads((root / ("fixtures/%s.json" % name)).read_text())
            response = json.loads((root / ("observations/%s.json" % name)).read_text())
            self.assertEqual(len(request["operations"]), len(response["results"]))
            for operation, result in zip(request["operations"], response["results"]):
                probe.validate(operation)
                self.assertEqual(operation["id"], result["id"])
                rows = result["value"]["steps"]
                self.assertEqual(len(operation["steps"]), len(rows))
                for tokens, row in zip(operation["steps"], rows):
                    self.assertEqual(tokens, row["tokens"])
                    self.assertEqual(probe.script(tokens), row["macro"])
                    self.assertIn("Choose coordinate system", row["history"])
                    checked += 1
        self.assertEqual(checked, 33)
