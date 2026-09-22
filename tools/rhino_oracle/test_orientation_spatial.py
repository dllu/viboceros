"""Bounded spatial witnesses, not a general orientation classifier or replay."""
import copy
import hashlib
import itertools
import json
import unittest

from .references.orientation_audit import spatial_request
from .test_orientation import ROOT, retained, reversed_brep


class SpatialOrientationTests(unittest.TestCase):
    def test_source_only_regeneration_hashes_and_complete_capture(self):
        request = spatial_request()
        self.assertEqual(request, retained("fixtures", "orientation_spatial"))
        metadata = json.loads((ROOT / "docs/orientation-spatial-provenance.json").read_text())
        for path, expected in metadata["retained_file_sha256"].items():
            self.assertEqual(hashlib.sha256((ROOT / path).read_bytes()).hexdigest(), expected)
        data = retained("observations", "orientation_spatial")
        self.assertEqual(data["engine_version"], metadata["engine_version"])
        self.assertEqual(len(data["results"]), metadata["completed_operations"])
        self.assertEqual(len(data["results"]), 24)
        self.assertEqual([r["id"] for r in data["results"]], [op["id"] for op in request["operations"]])

    def test_axis_translations_sizes_senses_and_incidence_are_retained(self):
        for op, row in zip(spatial_request()["operations"], retained("observations", "orientation_spatial")["results"]):
            with self.subTest(case=op["id"]):
                self.assertFalse(op["measure_volume"])
                value = row["value"]
                self.assertIsNone(value["command"])
                self.assertIsNone(value["replacement"])
                self.assertEqual(len(value["constructed"]), 1)
                source = value["constructed"][0]
                self.assertEqual(source["type"], "brep")
                self.assertTrue(source["solid"])
                self.assertIsNone(source["volume"])
                parts = op["sources"][0]["parts"]
                self.assertEqual(len(source["vertices"]), 16)
                self.assertEqual(len(source["faces"]), 12)
                self.assertEqual(len(source["edges"]), 24)
                for i, part in enumerate(parts):
                    corners = set(itertools.product(*[(c-part["size"], c+part["size"])
                                                     for c in part["translation"]]))
                    self.assertEqual({tuple(v["point"]) for v in source["vertices"][8*i:8*(i+1)]}, corners)
                    self.assertEqual([f["reversed"] for f in source["topology"]["faces"][6*i:6*(i+1)]],
                                     [part["reversed"]] * 6)
                    for face in source["faces"][6*i:6*(i+1)]:
                        self.assertEqual(face["definition"]["degree"], [1, 1])
                        for cp in face["definition"]["control_points"]:
                            self.assertIn(tuple(cp["point"]), corners)
                            self.assertEqual(cp["weight"], 1.)
                incidence = [[0, 0] for _ in source["topology"]["edges"]]
                for face in source["topology"]["faces"]:
                    for loop in face["loops"]:
                        for trim in loop["trims"]:
                            incidence[trim["edge"]][int(face["reversed"] ^ trim["reversed"])] += 1
                self.assertEqual(incidence, [[1, 1]] * 24)

    def test_these_disjoint_boxes_follow_unique_minimum_x_shell_not_separation_axis(self):
        # A fixture-level observation only: there are no X ties, curved surfaces,
        # intersecting components, or trimmed extrema in this batch.
        for op, row in zip(spatial_request()["operations"], retained("observations", "orientation_spatial")["results"]):
            with self.subTest(case=op["id"]):
                parts = op["sources"][0]["parts"]
                extremes = [p["translation"][0] - p["size"] for p in parts]
                self.assertNotEqual(*extremes)
                first = parts[extremes.index(min(extremes))]
                source = row["value"]["constructed"][0]
                self.assertEqual(source["orientation"], "Inward" if first["reversed"] else "Outward")
                expected = reversed_brep(source) if first["reversed"] else copy.deepcopy(source)
                expected["orientation"] = "Outward"
                self.assertEqual(row["value"]["inserted"], [expected])

    def test_unsampled_x_records_match_earlier_definitions_without_rewriting_old_volumes(self):
        old = {r["id"]: r["value"] for r in retained("observations", "orientation_compounds")["results"]}
        new = {r["id"]: r["value"] for r in retained("observations", "orientation_spatial")["results"]}
        for size, label in ((1, "negative"), (2, "positive")):
            for order in (False, True):
                previous = old["disjoint-opposed-" + label + ("-reverse-order" if order else "")]
                current = new["axis-x-size-%d-reversed-False-order-%s" % (size, order)]
                for state in ("constructed", "inserted"):
                    expected = copy.deepcopy(previous[state])
                    self.assertIsNotNone(expected[0]["volume"])
                    expected[0]["volume"] = None
                    self.assertEqual(current[state], expected)


if __name__ == "__main__":
    unittest.main()
