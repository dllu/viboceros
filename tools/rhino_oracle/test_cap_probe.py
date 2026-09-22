"""Cap fixture validation cannot touch document or GUI state."""
import unittest
import hashlib
import json
import math
from collections import Counter
from pathlib import Path
from . import cap_probe
from .references.cap_compounds import request as compound_request


class CapProbeTests(unittest.TestCase):
    def test_requires_shared_artifact_before_host_access(self):
        for path in (None, False, 12, [], ""):
            with self.subTest(path=path), self.assertRaisesRegex(ValueError, "shared source artifact"):
                cap_probe.run({"artifact_path": path}, None, {})

    def test_invalid_selection_topology_and_booleans_cannot_touch_host(self):
        for update in ({"preselect": 1}, {"reversed": "False"},
                       {"keep_faces": []}, {"keep_faces": [0, 0]},
                       {"keep_faces": [-1]}, {"keep_faces": [True]},
                       {"keep_faces": [0.5]}, {"keep_faces": "0"},
                       {"edge_order": [0, 0]}, {"edge_order": [-1]},
                       {"edge_order": [0, 2]}, {"edge_order": [True]},
                       {"edge_order": "0,1"}):
            with self.subTest(update=update), self.assertRaises(ValueError):
                cap_probe.run(dict({"artifact_path": "owned.3dm"}, **update), None, {})

    def test_valid_face_and_edge_orders_are_not_rewritten(self):
        operation = {"artifact_path": "owned.3dm", "preselect": True,
                     "keep_faces": [2, 0], "edge_order": [1, 0]}
        cap_probe.validate(operation)
        self.assertEqual(operation["keep_faces"], [2, 0])
        self.assertEqual(operation["edge_order"], [1, 0])


class CompoundCapEvidenceTests(unittest.TestCase):
    def test_compound_matrix_is_generated_without_observations(self):
        root = Path(__file__).parent
        fixture = json.loads((root / "fixtures/cap_compounds.json").read_text())
        self.assertEqual(fixture, compound_request())
        self.assertEqual(len(fixture["operations"]), 100)
        self.assertEqual(len({op["id"] for op in fixture["operations"]}), 100)
        repo = root.parents[1]
        provenance = json.loads((repo / "docs/cap-compound-provenance.json").read_text())
        for path, expected in provenance["retained_file_sha256"].items():
            self.assertEqual(hashlib.sha256((repo / path).read_bytes()).hexdigest(), expected)

    def test_box_integrals_and_document_state_have_independent_witnesses(self):
        root = Path(__file__).parent
        records = json.loads((root / "observations/cap_compounds.json").read_text())
        self.assertEqual(len(records["results"]), 100)
        ordinary, coincident = 0, 0
        for op, record in zip(compound_request()["operations"], records["results"]):
            with self.subTest(id=op["id"]):
                self.assertEqual(record["id"], op["id"])
                value = record["value"]
                self.assertTrue(value["succeeded"])
                self.assertEqual(len(value["objects"]), 1)
                obj = value["objects"][0]
                self.assertEqual({k: v for k, v in obj.items() if k != "geometry"}, {
                    "source": True, "name": "Source", "selected": op["preselect"],
                    "layer": "Source", "color": [11, 22, 33],
                    "color_source": "ColorFromObject", "group_count": 1,
                })
                before, after = value["input"], obj["geometry"]
                self.assertFalse(before["solid"])
                self.assertIsNone(before["volume"])
                parts = op["source"]["parts"]
                vertices, area_before, area_after, signed_volume = [], 0, 0, 0
                for part in parts:
                    box = part["source"]
                    low, high = box["min"], box["max"]
                    sides = [b-a for a, b in zip(low, high)]
                    self.assertEqual(sides, [sides[0]] * 3)
                    vertices.extend((x, y, z) for x in (low[0], high[0])
                                    for y in (low[1], high[1]) for z in (low[2], high[2]))
                    area_before += len(box.get("keep_faces", range(6))) * sides[0]**2
                    area_after += 6 * sides[0]**2
                    signed_volume += (-1 if part["reversed"] else 1) * math.prod(sides)
                self.assertEqual(Counter(map(tuple, before["vertices"])), Counter(vertices))
                self.assertAlmostEqual(sum(f[1] for f in before["faces"]), area_before, delta=1e-10)
                if op["id"].startswith("coincident-opposed-"):
                    self.assertFalse(after["solid"])
                    self.assertIsNone(after["volume"])
                    self.assertEqual(len(after["faces"]), 11)
                    self.assertEqual(len(after["edges"]), 32)
                    coincident += 1
                    continue
                # Unique leftmost box determines the whole-result sense. Its
                # root reversal cancels the final normalization reversal.
                leftmost = min(parts, key=lambda part: part["source"]["min"][0])
                minima = [part["source"]["min"][0] for part in parts]
                self.assertEqual(minima.count(min(minima)), 1)
                if leftmost["reversed"]:
                    signed_volume = -signed_volume
                self.assertTrue(after["solid"])
                self.assertTrue(after["manifold"])
                self.assertEqual(len(after["faces"]), 12)
                self.assertEqual(len(after["edges"]), 24)
                self.assertEqual(Counter(map(tuple, after["vertices"])), Counter(vertices))
                self.assertAlmostEqual(sum(f[1] for f in after["faces"]), area_after, delta=1e-10)
                self.assertAlmostEqual(after["volume"], signed_volume, delta=1e-10)
                ordinary += 1
        self.assertEqual((ordinary, coincident), (96, 4))
