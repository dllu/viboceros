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
            "intersection_circle_line_snaps": {
                "circle-alone": "None",
                "circle-line-right": "Intersection",
                "circle-line-left": "Intersection",
                "circle-line-perspective": "Intersection",
                "circle-line-tangent": "Intersection",
                "circle-line-apparent": "Intersection",
            },
            "intersection_circle_line_detail_snaps": {
                "perspective-circle-at-zero": "Intersection",
                "perspective-circle-at-two": "Intersection",
                "perspective-circle-at-four": "Intersection",
                "perspective-circle-at-left": "Intersection",
                "perspective-circle-at-below": "Intersection",
                "perspective-line-first": "Intersection",
                "perspective-tangent": "Intersection",
            },
            "intersection_arc_ellipse_snaps": {
                "arc-alone": "None",
                "arc-line-right": "Intersection",
                "arc-line-left": "Intersection",
                "arc-line-outside-sweep": "None",
                "arc-line-tangent": "Intersection",
                "arc-line-endpoint": "Intersection",
                "arc-line-perspective": "Intersection",
                "ellipse-line-right": "Intersection",
                "ellipse-line-left": "Intersection",
                "ellipse-line-tangent": "Intersection",
                "ellipse-line-perspective": "Intersection",
                "ellipse-line-apparent": "Intersection",
            },
            "intersection_arc_ellipse_priority_snaps": {
                "arc-line-tangent-reverse": "Intersection",
                "arc-line-endpoint-reverse": "Intersection",
                "arc-line-perspective-reverse": "Intersection",
                "ellipse-line-left-reverse": "Intersection",
                "ellipse-line-tangent-reverse": "Intersection",
                "ellipse-line-perspective-reverse": "Intersection",
            },
            "intersection_circle_circle_snaps": {
                "circle-circle-top-upper": "Intersection",
                "circle-circle-top-lower": "Intersection",
                "circle-circle-top-reverse": "Intersection",
                "circle-circle-perspective": "Intersection",
                "circle-circle-perspective-reverse": "Intersection",
                "circle-circle-apparent": "Intersection",
                "circle-circle-tangent": "Intersection",
                "circle-circle-disjoint": "None",
                "circle-circle-coincident": "Intersection",
            },
            "intersection_circle_circle_detail_snaps": {
                "coincident-upper": "Intersection",
                "coincident-left": "Intersection",
                "coincident-diagonal": "None",
                "coincident-perspective": "Intersection",
                "coincident-reverse": "Intersection",
                "tangent-reverse": "Intersection",
            },
            "intersection_circle_circle_overlap_snaps": {
                "coincident-bottom": "Intersection",
                "coincident-rotated-diagonal": "Intersection",
                "coincident-rotated-upper": "Intersection",
                "coincident-rotated-reverse-diagonal": "Intersection",
            },
            "intersection_circle_circle_seams_snaps": {
                "seam-30-at-30": "Intersection",
                "seam-30-upper": "Intersection",
                "seam-30-at-30-reverse": "Intersection",
                "seam-minus45-at-minus45": "Intersection",
                "seam-180-upper": "Intersection",
                "seam-180-right": "Intersection",
            },
            "intersection_circle_circle_quadrants_snaps": {
                "rotated-quarter-120": "Intersection",
                "rotated-quarter-210": "Intersection",
                "rotated-quarter-120-reverse": "Intersection",
            },
            "intersection_near_tangent_snaps": {
                "near-tangent-upper": "Intersection",
                "near-tangent-lower": "Intersection",
                "near-tangent-upper-reverse": "Intersection",
                "near-tangent-lower-reverse": "Intersection",
            },
            "intersection_near_tangent_cursor_snaps": {
                "near-tangent-upper-dxm4": "Intersection",
                "near-tangent-upper-dxm2": "Intersection",
                "near-tangent-upper-dx0": "Intersection",
                "near-tangent-upper-dx2": "Intersection",
                "near-tangent-upper-dx4": "Intersection",
                "near-tangent-lower-dxm4": "Intersection",
                "near-tangent-lower-dxm2": "Intersection",
                "near-tangent-lower-dx0": "Intersection",
                "near-tangent-lower-dx2": "Intersection",
                "near-tangent-lower-dx4": "Intersection",
            },
            "intersection_near_tangent_frames_snaps": {
                "near-tangent-upper-halfturn": "Intersection",
                "near-tangent-lower-halfturn": "Intersection",
                "near-tangent-upper-quarterturn": "Intersection",
                "near-tangent-lower-quarterturn": "Intersection",
            },
            "intersection_near_tangent_vertical_snaps": {
                "vertical-near-tangent-upper": "Intersection",
                "vertical-near-tangent-lower": "Intersection",
                "vertical-near-tangent-upper-reverse": "Intersection",
                "vertical-near-tangent-lower-reverse": "Intersection",
            },
            "intersection_conic_pairs_snaps": {
                "circle-ellipse-ne": "Intersection",
                "circle-ellipse-nw": "Intersection",
                "circle-ellipse-sw": "Intersection",
                "circle-ellipse-perspective": "Intersection",
                "circle-ellipse-tangent": "Intersection",
                "circle-arc-upper": "Intersection",
                "circle-arc-lower": "None",
                "ellipse-ellipse-upper": "Intersection",
                "ellipse-ellipse-tangent": "Intersection",
            },
            "intersection_nurbs_line_snaps": {
                "quadratic-left": "Intersection",
                "quadratic-right": "Intersection",
                "quadratic-tangent": "Intersection",
                "quadratic-miss": "None",
                "quadratic-apparent": "Intersection",
                "quadratic-perspective": "Intersection",
                "quadratic-left-reverse": "Intersection",
                "rational-left": "Intersection",
            },
            "intersection_nurbs_spans_snaps": {
                "continuous-second-span": "Intersection",
            },
            "intersection_nurbs_conic_snaps": {
                "nurbs-circle-left": "Intersection",
                "nurbs-circle-right": "Intersection",
                "nurbs-circle-left-reverse": "Intersection",
                "nurbs-circle-perspective": "Intersection",
                "nurbs-circle-apparent": "Intersection",
                "nurbs-circle-tangent": "Intersection",
                "nurbs-circle-disjoint": "None",
                "nurbs-ellipse-right": "Intersection",
            },
            "intersection_nurbs_arc_snaps": {
                "nurbs-arc-right": "Intersection",
                "nurbs-arc-left": "None",
                "nurbs-arc-right-reverse": "Intersection",
            },
            "intersection_nurbs_conic_rational_snaps": {
                "rational-nurbs-circle-left": "Intersection",
            },
            "intersection_nurbs_pair_snaps": {
                "nurbs-pair-left": "Intersection",
                "nurbs-pair-right": "Intersection",
                "nurbs-pair-left-reverse": "Intersection",
                "nurbs-pair-apparent": "Intersection",
                "nurbs-pair-perspective": "Intersection",
                "nurbs-pair-tangent": "Intersection",
                "nurbs-pair-disjoint": "None",
            },
            "intersection_nurbs_pair_detail_snaps": {
                "rational-nurbs-pair-left": "Intersection",
                "multispan-nurbs-pair-right": "Intersection",
            },
            "intersection_nurbs_self_snaps": {
                "self-cubic-planar": "Intersection",
                "self-cubic-depth": "Intersection",
                "self-cubic-depth-reverse": "Intersection",
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
