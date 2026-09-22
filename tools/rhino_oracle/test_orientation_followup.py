"""Closed incidence is not consistent orientation; retain anomalous transcripts."""
import json
import hashlib
from fractions import Fraction
import unittest

from .references.orientation_audit import face_request, compound_request
from .test_orientation import ROOT, retained, reversed_brep


def require_clean_flip_history(command):
    for event in command["events"]:
        if event["name"] not in ("Flip", "SelID", "Enter"):
            raise ValueError("unrelated event in Flip capture")
    for line in command["history"].splitlines():
        if line.startswith("Command: ") and line[len("Command: "):].strip() not in ("_Flip", "_Enter"):
            raise ValueError("unrelated trailing command in Flip transcript")


class OrientationFollowupTests(unittest.TestCase):
    def test_retained_followup_hashes_and_complete_batches(self):
        metadata = json.loads((ROOT / "docs/orientation-followup-provenance.json").read_text())
        for path, expected in metadata["retained_file_sha256"].items():
            self.assertEqual(hashlib.sha256((ROOT / path).read_bytes()).hexdigest(), expected)
        for name, key, count in (("orientation_compounds", "primary_compound_cases", 20),
                                 ("orientation_faces", "primary_inconsistent_face_cases", 16),
                                 ("orientation_faces_startup_recovery", "diagnostic_face_repeat_cases", 16)):
            data = retained("observations", name)
            self.assertEqual(data["engine_version"], metadata["engine_version"])
            self.assertEqual(len(data["results"]), count)
            self.assertEqual(metadata[key], count)

    def test_compound_volume_witnesses_and_full_insertion_replacement_records(self):
        request = compound_request()
        self.assertEqual(retained("fixtures", "orientation_compounds"), request)
        observed = retained("observations", "orientation_compounds")
        self.assertEqual([r["id"] for r in observed["results"]], [op["id"] for op in request["operations"]])
        for op, row in zip(request["operations"], observed["results"]):
            value = row["value"]
            source = value["constructed"][0]
            parts = op["sources"][0]["parts"]
            # Independent integral of axis-aligned boxes: side length 2r,
            # volume (2r)^3. Neither native output nor observed scalars enter it.
            exact_volume = sum((Fraction(8) * p["size"] ** 3 * (-1 if p["reversed"] else 1)
                                for p in parts), Fraction(0))
            self.assertLess(abs(source["volume"] - float(exact_volume)), 1e-10)
            self.assertEqual([f["reversed"] for f in source["topology"]["faces"]],
                             [p["reversed"] for p in parts for _ in range(6)])
            self.assertTrue(source["solid"])
            self.assertIn(source["orientation"], ("Outward", "Inward"))
            if source["orientation"] == "Inward":
                expected = reversed_brep(source)
                expected["orientation"] = "Outward"
                expected["volume"] = -source["volume"]
            else:
                expected = source
            self.assertEqual(value["inserted"], [expected])
            self.assertEqual(value["replacement"][0]["geometry"], expected)
            command = value["command"]
            require_clean_flip_history(command)
            self.assertTrue(command["succeeded"])
            self.assertEqual([e["result"] for e in command["events"] if e["name"] == "Flip"], ["Success"])
            self.assertEqual(command["objects"][0]["geometry"], expected)
            self.assertTrue(command["objects"][0]["selected"])

    def test_volume_sign_and_component_table_order_cannot_replace_spatial_orientation(self):
        rows = {r["id"]: r["value"] for r in retained("observations", "orientation_compounds")["results"]}
        for suffix in ("", "-reverse-order"):
            value = rows["disjoint-opposed-negative" + suffix]
            self.assertEqual(value["constructed"][0]["orientation"], "Outward")
            self.assertLess(value["constructed"][0]["volume"], -55.)
            self.assertEqual(value["constructed"], value["inserted"])
        first = rows["coincident-opposed"]
        second = rows["coincident-opposed-reverse-order"]
        self.assertEqual(first["constructed"][0]["volume"], 0.)
        self.assertEqual(second["constructed"][0]["volume"], 0.)
        self.assertEqual(first["constructed"][0]["orientation"], "Inward")
        self.assertEqual(second["constructed"][0]["orientation"], "Outward")
        self.assertNotEqual(first["constructed"], first["inserted"])
        self.assertEqual(second["constructed"], second["inserted"])

    def test_face_sources_are_regenerated_without_observed_values(self):
        self.assertEqual(retained("fixtures", "orientation_faces"), face_request())

    def test_clean_closed_unoriented_cases_preserve_complete_geometry_and_selection(self):
        data = retained("observations", "orientation_faces")
        operations = face_request()["operations"]
        self.assertEqual([r["id"] for r in data["results"]], [op["id"] for op in operations])
        for op, row in zip(operations, data["results"]):
            value = row["value"]
            source = value["constructed"][0]
            counts = [[0, 0] for _ in source["topology"]["edges"]]
            for face in source["topology"]["faces"]:
                for loop in face["loops"]:
                    for trim in loop["trims"]:
                        counts[trim["edge"]][int(face["reversed"] ^ trim["reversed"])] += 1
            self.assertTrue(all(sum(pair) == 2 for pair in counts))
            self.assertTrue(any(pair != [1, 1] for pair in counts))
            self.assertFalse(source["solid"])
            self.assertEqual(source["orientation"], "None")
            spec = op["sources"][0]
            self.assertEqual([f["reversed"] for f in source["topology"]["faces"]],
                             [(i in spec["flipped_faces"]) ^ spec["reversed"] for i in range(6)])
            self.assertEqual(value["inserted"], [source])
            self.assertEqual(value["replacement"][0]["geometry"], reversed_brep(source))
            command = value["command"]
            require_clean_flip_history(command)
            self.assertTrue(command["succeeded"])
            endings = [e for e in command["events"] if e["name"] == "Flip"]
            self.assertEqual(len(endings), 1)
            self.assertEqual(endings[0]["result"], "Success")
            self.assertEqual(len(command["objects"]), 1)
            obj = command["objects"][0]
            self.assertEqual(obj["geometry"], source)
            self.assertEqual(obj["selected"], op["preselect"])
            self.assertEqual(obj["name"], "source-0")
            self.assertEqual(obj["groups"], 1)
            self.assertTrue(obj["current_layer"])

    def test_startup_recovery_record_is_retained_but_not_counted_as_clean_evidence(self):
        previous = retained("observations", "orientation_faces_startup_recovery")
        clean = retained("observations", "orientation_faces")
        self.assertEqual(len(previous["results"]), 16)
        self.assertEqual(len(clean["results"]), 16)
        with self.assertRaises(ValueError):
            require_clean_flip_history(previous["results"][0]["value"]["command"])
        for old, new in zip(previous["results"], clean["results"]):
            self.assertEqual(old["id"], new["id"])
            for key in ("constructed", "inserted", "replacement"):
                self.assertEqual(old["value"][key], new["value"][key])
            # Even the contaminated record's geometry agrees, but that does not
            # license silently stripping its extraneous history commands.
            self.assertEqual(old["value"]["command"]["objects"], new["value"]["command"]["objects"])
        for row in previous["results"][1:]:
            require_clean_flip_history(row["value"]["command"])


if __name__ == "__main__":
    unittest.main()
