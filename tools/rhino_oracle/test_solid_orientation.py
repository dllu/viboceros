"""Same-source observations remain independent of the incomplete classifier."""
import hashlib
import json
from pathlib import Path
import unittest

from .references.solid_orientation import request

ROOT = Path(__file__).resolve().parents[2]


class SolidOrientationTests(unittest.TestCase):
    def test_source_regeneration_and_retained_capture_hashes(self):
        fixture = json.loads((ROOT / "tools/rhino_oracle/fixtures/solid_orientation.json").read_text())
        self.assertEqual(fixture, request())
        provenance = json.loads((ROOT / "docs/solid-orientation-provenance.json").read_text())
        for path, expected in provenance["retained_file_sha256"].items():
            self.assertEqual(hashlib.sha256((ROOT / path).read_bytes()).hexdigest(), expected)
        observed = json.loads((ROOT / "tools/rhino_oracle/observations/solid_orientation.json").read_text())
        self.assertEqual(observed["engine_version"], provenance["engine_version"])
        self.assertEqual(len(observed["results"]), 49)
        self.assertEqual([r["id"] for r in observed["results"]], [o["id"] for o in fixture["operations"]])

    def test_unique_box_extrema_match_analytic_face_sense_but_ties_are_representation_specific(self):
        observed = json.loads((ROOT / "tools/rhino_oracle/observations/solid_orientation.json").read_text())
        rows = {r["id"]: r["value"] for r in observed["results"]}
        for operation in request()["operations"][:44]:
            sources = operation["sources"]
            minimum = min(s["source"]["min"][0] for s in sources)
            senses = {s["reversed"] for s in sources if s["source"]["min"][0] == minimum}
            if len(senses) == 1:
                expected = "Inward" if senses.pop() else "Outward"
                self.assertEqual(rows[operation["id"]]["orientation"], expected)
            self.assertTrue(rows[operation["id"]]["solid"])
            self.assertTrue(rows[operation["id"]]["closed"])
        for key in ("open-box", "unoriented-1", "unoriented-3"):
            self.assertEqual(rows[key]["orientation"], "None")
            self.assertFalse(rows[key]["solid"])
            self.assertEqual(rows[key]["closed"], key != "open-box")
        self.assertEqual(rows["corner-False"]["orientation"], "Outward")
        self.assertEqual(rows["corner-True"]["orientation"], "Inward")
        self.assertEqual(rows["coincident-opposed"]["orientation"], "Outward")
        self.assertEqual(rows["coincident-opposed-reverse-order"]["orientation"], "Inward")
        previous = json.loads((ROOT / "tools/rhino_oracle/observations/orientation_compounds.json").read_text())
        previous = {r["id"]: r["value"]["constructed"][0] for r in previous["results"]}
        for key in ("coincident-opposed", "coincident-opposed-reverse-order"):
            # Same geometric box recipes, but different representations: do not
            # turn either batch into a representation-independent order rule.
            self.assertNotEqual(previous[key]["orientation"], rows[key]["orientation"])
            self.assertNotEqual(previous[key]["faces"], rows[key]["geometry"]["faces"])


if __name__ == "__main__":
    unittest.main()
