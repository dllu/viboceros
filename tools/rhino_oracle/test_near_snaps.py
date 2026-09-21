"""Near calibration integrity and independent projected closest-point witnesses.

These are reference-math checks, not native snapping or command replay claims.
SplitEdge exposes only the snapped target's x coordinate in these box fixtures.
"""
import copy
import hashlib
import json
import math
from pathlib import Path
import unittest

from . import split_edge_probe
from .references import near_snaps, projected_near

ROOT = Path(__file__).resolve().parents[2]


class NearSnapTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.request = json.loads((ROOT / "tools/rhino_oracle/fixtures/near_snaps.json").read_text())
        cls.observed = json.loads((ROOT / "tools/rhino_oracle/observations/near_snaps.json").read_text())

    def pairs(self):
        return zip(self.request["operations"], self.observed["results"])

    def test_retained_hashes_and_independent_generator(self):
        provenance = json.loads((ROOT / "docs/near-snaps-provenance.json").read_text())
        for path, digest in provenance["retained_file_sha256"].items():
            self.assertEqual(hashlib.sha256((ROOT / path).read_bytes()).hexdigest(), digest)
        self.assertEqual(near_snaps.request(), self.request)
        # A caller cannot contaminate the next generated request.
        changed = near_snaps.request()
        changed["operations"][0]["sources"][1]["start"][0] = 999
        self.assertEqual(near_snaps.request(), self.request)
        self.assertEqual(len(self.request["operations"]), 16)
        self.assertEqual(len(self.observed["results"]), 16)
        self.assertEqual(self.observed["engine_version"], "8.32.26160.13001")

    def test_real_prompt_calibration_and_complete_undo_redo_are_retained(self):
        for operation, row in self.pairs():
            with self.subTest(id=operation["id"]):
                self.assertEqual(operation["id"], row["id"])
                owned = copy.deepcopy(operation)
                owned["sources"][0]["brep"]["artifact_path"] = "/owned/box.3dm"
                split_edge_probe.validate(owned)
                value = row["value"]
                self.assertTrue(value["succeeded"] and value["history_tested"])
                self.assertEqual(value["before"], value["undo"])
                self.assertEqual(value["after"], value["redo"])
                self.assertEqual(value["before"][1:], value["after"][1:])
                self.assertEqual(value["command_history"].count("_Pause"), 2)
                pick = operation["inputs"][0]["pick"]
                self.assertEqual(value["command_history"].count("_Near"), int(pick["osnap"] == "Near"))
                self.assertEqual(len(value["pick_frames"]), 1)
                frame = value["pick_frames"][0]
                self.assertEqual(frame["aim"], pick["aim"])
                self.assertTrue(frame["perspective"])
                self.assertEqual(len(frame["world_to_screen"]), 4)
                self.assertTrue(all(len(r) == 4 for r in frame["world_to_screen"]))
                self.assertTrue(all(math.isfinite(v) for r in frame["world_to_screen"] for v in r))
                self.assertAlmostEqual(sum(v*v for v in frame["camera_direction"]), 1., places=12)
                pixel = projected_near.project(frame, pick["aim"])
                for axis in range(2):
                    self.assertLess(abs(pixel[axis] - frame["aim_client"][axis]), 1e-7)
                    click = frame["click_client"][axis]
                    self.assertIs(type(click), int)
                    self.assertEqual(click, int(frame["aim_client"][axis]) + pick["offset"][axis])
                    self.assertTrue(1 <= click < frame["size"][axis] - 1)
                before = value["before"][0]["geometry"]["brep"]["vertices"]
                after = value["after"][0]["geometry"]["brep"]["vertices"]
                self.assertEqual(after[:-1], before)
                self.assertEqual(after[-1][1:], [0., 0.])

    def test_reference_sources_stay_entirely_in_front_of_the_projection_plane(self):
        for operation, row in self.pairs():
            source, frame = operation["sources"][1], row["value"]["pick_frames"][0]
            if source["type"] == "circle":
                # A containing box suffices: w is affine in world coordinates.
                points = [[source["center"][i] + (-1. if bits & (1 << i) else 1.)*source["radius"]
                           for i in range(3)] for bits in range(8)]
            elif source["type"] == "nurbs":
                self.assertTrue(all(c["weight"] > 0. for c in source["control_points"]))
                points = [c["point"] for c in source["control_points"]]
            elif source["type"] == "polyline":
                points = source["vertices"]
            else:
                points = [source["start"], source["end"]]
            for point in points:
                self.assertGreater(projected_near.homogeneous(frame["world_to_screen"], point)[3], 0.)
                self.assertGreater(sum((point[i]-frame["camera_location"][i])*frame["camera_direction"][i]
                                       for i in range(3)), 0.)

    def test_reference_targets_predict_the_observed_constrained_coordinate(self):
        near_count, feature_count = 0, 0
        explicit = {"line-end": [2., -2., 0.], "line-mid": [5., -2., 0.],
                    "circle-quad": [2., -4., 0.]}
        for operation, row in self.pairs():
            with self.subTest(id=operation["id"]):
                frame = row["value"]["pick_frames"][0]
                # Neither declared pick.point nor any result geometry is passed
                # to the reference: just source geometry and the actual camera/click.
                near = projected_near.near_point(operation["sources"][1], frame)
                self.assertLess(projected_near.distance(frame, near), 12.)
                expected = explicit.get(operation["id"], near)
                observed = row["value"]["after"][0]["geometry"]["brep"]["vertices"][-1]
                self.assertLess(abs(observed[0] - expected[0]), 1e-9)
                if operation["id"] in explicit:
                    feature_count += 1
                    self.assertLess(projected_near.distance(frame, expected), 12.)
                    self.assertGreater(projected_near.distance(frame, expected),
                                       projected_near.distance(frame, near) + 1e-4)
                    self.assertGreater(abs(near[0] - expected[0]), 1e-3)
                else:
                    near_count += 1
                    self.assertGreater(abs(near[0] - operation["inputs"][0]["pick"]["point"][0]), 1e-3)
                if operation["id"] == "circle-center":
                    self.assertGreater(abs(observed[0] - operation["sources"][1]["center"][0]), 1.)
        self.assertEqual((near_count, feature_count), (13, 3))

    def test_one_shot_and_persistent_near_agree_for_all_six_families(self):
        rows = {row["id"]: row["value"] for row in self.observed["results"]}
        for family in ("line", "line-off-plane", "line-tilted", "polyline", "circle", "ellipse-quarter"):
            a, b = [rows[family + suffix]["after"][0]["geometry"]["brep"]["vertices"][-1][0]
                    for suffix in ("-one-shot", "-persistent")]
            self.assertLess(abs(a-b), 1e-12)

    def test_projective_line_reference_does_not_interpolate_screen_fraction_in_world_space(self):
        frame = dict(world_to_screen=[[1., 0., 0., 0.], [0., 1., 0., 0.],
                                      [0., 0., 1., 0.], [0., 0., 1., 0.]],
                     click_client=[.5, .1])
        a, b = [0., 0., 1.], [2., 0., 2.]
        point = projected_near.line_point(frame, a, b)
        for actual, expected in zip(point, [2./3., 0., 4./3.]):
            self.assertAlmostEqual(actual, expected, places=14)
        self.assertAlmostEqual(projected_near.distance(frame, point), .1, places=14)
        self.assertGreater(projected_near.distance(frame, [1., 0., 1.5]), .19)
        for cursor, endpoint in (([-1., 0.], a), ([2., 0.], b)):
            self.assertEqual(projected_near.line_point(dict(frame, click_client=cursor), a, b), endpoint)
        with self.assertRaises(ValueError):
            projected_near.line_point(frame, [0., 0., -1.], b)

    def test_circle_reference_retains_height_and_analytic_tangent_is_independent(self):
        source = dict(type="circle", center=[0., 0., 7.], radius=2.,
                      x_axis=[1., 0., 0.], normal=[0., 0., 1.])
        frame = dict(world_to_screen=[[1., 0., 0., 0.], [0., 1., 0., 0.],
                                      [0., 0., 1., 0.], [0., 0., 0., 1.]],
                     click_client=[1.8, 2.4])
        for actual, expected in zip(projected_near.near_point(source, frame), [1.2, 1.6, 7.]):
            self.assertAlmostEqual(actual, expected, places=12)
        source = near_snaps.request()["operations"][10]["sources"][1]
        for t in (.1, .4, .8):
            _, tangent = projected_near.curve_jet(source, t)
            a = projected_near.curve_jet(source, t-1e-5)[0]
            b = projected_near.curve_jet(source, t+1e-5)[0]
            for axis in range(3):
                self.assertLess(abs((b[axis]-a[axis])/2e-5 - tangent[axis]), 1e-8)


if __name__ == "__main__":
    unittest.main()
