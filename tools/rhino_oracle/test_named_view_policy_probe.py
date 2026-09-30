"""Bounds and independent cleanup for disposable named-view oracle captures."""
import json
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import patch

from . import named_view_policy_probe as probe


def fixture():
    return json.loads((Path(__file__).parent / "fixtures/named_view_policy.json").read_text())["operations"][0]


class NamedViewPolicyProbeTests(unittest.TestCase):
    def test_validation_bounds_mutations_before_any_live_operations(self):
        probe.validate(fixture())
        for change in [
            dict(op="NamedView"), dict(saved_target=[float("nan"), 0, 0]),
            dict(current_target=[0, True, 0]), dict(saved_origin=[0, 0]),
            dict(view_policy={}), dict(view_policy=dict(set_cplane=1, set_projection=True)),
            dict(view_policy=dict(set_cplane=True, set_projection=True, other=True)),
            dict(saved_projections=[]), dict(current_projections=["Top", "Top"]),
            dict(current_projections=["Front"]), dict(saved_frustum_scale=0),
            dict(saved_frustum_scale=True), dict(saved_frustum_scale=float("inf")),
            dict(saved_frustum_shift=[4, 0]), dict(saved_frustum_shift=[0, float("nan")]),
            dict(saved_frustum_shift=[True, 0]), dict(saved_frustum_shift=[0]),
        ]:
            with self.subTest(change=change), self.assertRaises(ValueError):
                probe.validate(dict(fixture(), **change))

    def test_owned_views_and_settings_are_restored_even_when_cleanup_fails(self):
        for cleanup_failure in [False, True]:
            with self.subTest(cleanup_failure=cleanup_failure):
                settings = SimpleNamespace(DefinedViewSetCPlane=False, DefinedViewSetProjection=False)
                original_target, original_plane = object(), object()
                state = dict(deleted=[], disposed=False, projection=None, plane=None,
                             target=None, initializes=0, restore_policy=None)
                viewport = SimpleNamespace(Name="Original", CameraTarget=original_target, Id="viewport")

                def initialize(projection, name, redraw):
                    self.assertTrue(settings.DefinedViewSetCPlane)
                    self.assertTrue(settings.DefinedViewSetProjection)
                    state["initializes"] += 1
                    viewport.Name = name
                    return True

                def set_target(target, redraw):
                    state["target"] = target
                    return True

                def set_plane(plane):
                    state["plane"] = plane
                    return True

                viewport.SetProjection = initialize
                viewport.SetCameraTarget = set_target
                viewport.SetConstructionPlane = set_plane
                viewport.ConstructionPlane = lambda: original_plane

                class Info:
                    def __init__(self, view):
                        pass

                    def Dispose(self):
                        state["disposed"] = True

                def restore_projection(info, redraw):
                    state["projection"] = info
                    return True

                viewport.SetViewProjection = restore_projection

                def restore(index, view):
                    self.assertEqual(index, 6)
                    self.assertIs(view, viewport)
                    state["restore_policy"] = (settings.DefinedViewSetCPlane, settings.DefinedViewSetProjection)
                    return False

                def delete(index):
                    state["deleted"].append(index)
                    return not cleanup_failure

                table = SimpleNamespace(Add=lambda name, id: 6, Restore=restore, Delete=delete)
                vector = lambda *args: args
                Rhino = SimpleNamespace(
                    ApplicationSettings=SimpleNamespace(ViewSettings=settings),
                    RhinoDoc=SimpleNamespace(ActiveDoc=SimpleNamespace(NamedViews=table)),
                    DocObjects=SimpleNamespace(ViewportInfo=Info),
                    Display=SimpleNamespace(DefinedViewportProjection=SimpleNamespace(Top="Top")),
                    Geometry=SimpleNamespace(Point3d=vector, Vector3d=vector,
                        Plane=lambda *args: SimpleNamespace(IsValid=True)),
                )
                operation = dict(fixture(), saved_projections=["Top"], current_projections=["Top"],
                                 view_policy=dict(set_cplane=True, set_projection=False))
                message = "cleanup failed" if cleanup_failure else "could not restore"
                with patch.object(probe, "snapshot", return_value={}):
                    with self.assertRaisesRegex(ValueError, message):
                        probe.run(operation, viewport, dict(Rhino=Rhino, progress=lambda text: None))
                self.assertEqual(state["deleted"], [6])
                self.assertEqual(state["initializes"], 2)
                self.assertEqual(state["restore_policy"], (True, False))
                self.assertFalse(settings.DefinedViewSetCPlane)
                self.assertFalse(settings.DefinedViewSetProjection)
                self.assertIs(state["target"], original_target)
                self.assertIs(state["plane"], original_plane)
                self.assertIsInstance(state["projection"], Info)
                self.assertEqual(viewport.Name, "Original")
                self.assertTrue(state["disposed"])

    def test_saved_capture_includes_all_policies_and_projection_pairs(self):
        root = Path(__file__).parent
        request = json.loads((root / "fixtures/named_view_policy.json").read_text())
        capture = json.loads((root / "observations/named_view_policy.json").read_text())
        self.assertEqual(len(capture["results"]), len(request["operations"]))
        count = 0
        for operation, result in zip(request["operations"], capture["results"]):
            probe.validate(operation)
            self.assertEqual(operation["id"], result["id"])
            pairs = {(row["saved_projection"], row["current_projection"]) for row in result["value"]}
            self.assertEqual(pairs, {(a, b) for a in operation["saved_projections"]
                                    for b in operation["current_projections"]})
            for row in result["value"]:
                self.assertEqual(row["view_policy"], operation["view_policy"])
                for prefix, projection in [("saved", row["saved_projection"]),
                                           ("before", row["current_projection"])]:
                    self.assertEqual(row[prefix]["perspective"], projection != "Top")
                    self.assertEqual(row[prefix]["two_point_perspective"], projection == "TwoPointPerspective")
                self.assertEqual(len(row["after"]["projected_points"]), 3)
                count += 1
        self.assertEqual(count, 72)


if __name__ == "__main__":
    unittest.main()
