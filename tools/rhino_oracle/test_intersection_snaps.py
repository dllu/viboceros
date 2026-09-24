"""Owned Rhino GetPoint evidence for straight intersection object snaps."""

import json
import unittest
from pathlib import Path

from .point_snap_probe import validate_request


ROOT = Path(__file__).resolve().parents[2]


class IntersectionSnapTests(unittest.TestCase):
    def test_retained_picks_keep_sources_settings_and_actual_snap_events(self):
        cases = {
            "intersection_snaps": {
                "single-line": "None",
                "cross-lines": "Intersection",
                "cross-perspective": "Intersection",
                "skew-top": "Intersection",
                "endpoint-touch": "Intersection",
                "overlap-lines": "None",
                "mesh-line-off": "None",
                "mesh-line-on": "Intersection",
            },
            "intersection_depth_snaps": {
                "skew-top-reverse": "Intersection",
                "skew-perspective": "None",
                "skew-perspective-reverse": "None",
            },
            "intersection_perspective_snaps": {
                "skew-perspective-projected": "Intersection",
            },
            "intersection_perspective_reverse_snaps": {
                "skew-perspective-projected-reverse": "Intersection",
            },
            "intersection_mixed_snaps": {
                "int-Near": "Intersection",
                "int-Mid": "Intersection",
                "int-End": "Intersection",
                "int-Point": "Intersection",
            },
            "intersection_competing_snaps": {
                "int-vs-unrelated-end": "End",
                "int-vs-unrelated-near": "Intersection",
            },
            "intersection_competing_mid_snaps": {
                "int-vs-unrelated-mid": "Midpoint",
                "int-vs-unrelated-end-close": "End",
            },
            "intersection_self_snaps": {
                "self-cross-polyline": "Intersection",
                "adjacent-polyline-corner": "Intersection",
                "self-cross-spatial-polyline": "Intersection",
            },
            "intersection_mesh_self_snaps": {
                "mesh-corner-off": "None",
                "mesh-corner-on": "None",
            },
        }
        for stem, expected in cases.items():
            with self.subTest(stem=stem):
                fixture = json.loads((ROOT / "tools/rhino_oracle/fixtures" / (stem + ".json")).read_text())
                observed = json.loads((ROOT / "tools/rhino_oracle/observations" / (stem + ".json")).read_text())
                validate_request(fixture)
                self.assertEqual([op["id"] for op in fixture["operations"]], list(expected))
                self.assertEqual([row["id"] for row in observed["results"]], list(expected))
                for operation, row in zip(fixture["operations"], observed["results"]):
                    value = row["value"]
                    self.assertEqual(value["kind"], expected[row["id"]])
                    self.assertEqual(value["before"], value["after"])
                    setting = value["mesh_snap_setting"]
                    self.assertEqual(setting["requested"], operation["snap_to_meshes"])
                    self.assertEqual(setting["before"], setting["restored"])
                    if value["kind"] == "None":
                        self.assertIsNone(value["source"])
                    else:
                        self.assertIsInstance(value["source"], int)


if __name__ == "__main__":
    unittest.main()
