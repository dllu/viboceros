"""Analytic source witnesses and complete planar-solid Rhino observations."""
from fractions import Fraction
import copy
import hashlib
import json
from pathlib import Path
import unittest

from .references.solid_orientation_polyhedra import mapped, request, shapes, translation_repeat_request

ROOT = Path(__file__).resolve().parents[2]


def volume(source):
    points = [[Fraction(c) for c in p] for p in source["vertices"]]
    result = Fraction(0)
    for face in source["faces"]:
        a = points[face[0]]
        for i in range(1,len(face)-1):
            b,c = points[face[i]],points[face[i+1]]
            result += sum(a[j]*(b[(j+1)%3]*c[(j+2)%3]-b[(j+2)%3]*c[(j+1)%3]) for j in range(3))/6
    return result


class PolyhedralOrientationTests(unittest.TestCase):
    def test_source_only_regeneration_and_independent_exact_volumes(self):
        fixture = json.loads((ROOT / "tools/rhino_oracle/fixtures/solid_orientation_polyhedra.json").read_text())
        self.assertEqual(fixture, request())
        self.assertEqual(len(fixture["operations"]),68)
        # These individual embedded shells have analytic volume signs. This
        # integral is a source check, not the native classifier's algorithm.
        expected = dict(tetrahedron=Fraction(5,6),box=8,concave_prism=14,square_tube=24)
        for name, shape in shapes().items():
            for transform in ("identity","shear","reflection","translated"):
                source = dict(shape,vertices=[mapped(p,transform) for p in shape["vertices"]])
                determinant = 1 if transform=="identity" else (-42 if transform=="reflection" else 42)
                self.assertEqual(volume(source),expected[name]*determinant)

    def test_complete_capture_hashes_and_explicit_translation_discrepancies(self):
        metadata = json.loads((ROOT / "docs/planar-orientation-provenance.json").read_text())
        for path, expected in metadata["retained_file_sha256"].items():
            self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(),expected)
        observed = json.loads((ROOT / "tools/rhino_oracle/observations/solid_orientation_polyhedra.json").read_text())
        self.assertEqual(observed["engine_version"],metadata["engine_version"])
        self.assertEqual(len(observed["results"]),68)
        differences = []
        for operation, row in zip(request()["operations"],observed["results"]):
            self.assertEqual(row["id"],operation["id"])
            sources = operation["sources"]
            if len(sources)==1:
                inward = sources[0]["reversed"] ^ ("-reflection-" in operation["id"])
            else:
                # Geometric extrema, not the tetrahedron's untrimmed surface
                # extension at X=-1 or the compound's summed signed volume.
                leftmost = next(s for s in sources if s["source"]["type"]=="box")
                inward = leftmost["reversed"]
            value = row["value"]
            self.assertTrue(value["solid"])
            self.assertTrue(value["closed"])
            exact = "Inward" if inward else "Outward"
            differing = "-translated-" in operation["id"] and (operation["id"].startswith(("tetrahedron-","square_tube-"))
                         or operation["id"].startswith("box-") and operation["id"].endswith("order-True"))
            self.assertEqual(value["orientation"]!=exact,differing,operation["id"])
            if differing: differences.append(operation["id"])
        self.assertEqual(len(differences),10)
        self.assertEqual(differences,metadata["primary_orientation_difference_ids"])

    def test_full_recorded_geometries_are_exact_translates_not_merely_similar_boxes(self):
        observed = json.loads((ROOT / "tools/rhino_oracle/observations/solid_orientation_polyhedra.json").read_text())
        rows = {r["id"]:r["value"] for r in observed["results"]}

        def positions(geometry):
            for vertex in geometry["vertices"]: yield vertex["point"]
            for edge in geometry["edges"]:
                for control in edge["curve"]["definition"]["control_points"]: yield control["point"]
            for face in geometry["faces"]:
                for control in face["definition"]["control_points"]: yield control["point"]

        for key,value in rows.items():
            if "-translated-" not in key: continue
            centered = rows[key.replace("-translated-","-shear-")]
            translated = copy.deepcopy(value["geometry"])
            a,b = list(positions(translated)),list(positions(centered["geometry"]))
            self.assertEqual(len(a),len(b))
            for current, expected in zip(a,b):
                self.assertEqual([Fraction(c)-t for c,t in zip(current,[2**40,-2**40,2**40])],
                                 [Fraction(c) for c in expected])
                current[:] = expected
            self.assertEqual(translated,centered["geometry"],key)

    def test_fresh_session_repeat_retains_the_same_geometry_and_orientation_discrepancies(self):
        fixture = json.loads((ROOT / "tools/rhino_oracle/fixtures/solid_orientation_translation_repeat.json").read_text())
        self.assertEqual(fixture,translation_repeat_request())
        primary = json.loads((ROOT / "tools/rhino_oracle/observations/solid_orientation_polyhedra.json").read_text())
        repeated = json.loads((ROOT / "tools/rhino_oracle/observations/solid_orientation_translation_repeat.json").read_text())
        self.assertEqual(len(repeated["results"]),8)
        self.assertEqual([r["id"] for r in repeated["results"]],[o["id"] for o in fixture["operations"]])
        rows = {r["id"]:r["value"] for r in primary["results"]}
        for row in repeated["results"]: self.assertEqual(row["value"],rows[row["id"]])


if __name__ == "__main__":
    unittest.main()
