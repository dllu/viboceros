"""Retained public-API/command observations, not a native geometry replay."""
import copy
import hashlib
import json
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

from .orientation_probe import validate, geometry_record
from .references.orientation_audit import request, compound_request, face_request, spatial_request

ROOT = Path(__file__).resolve().parents[2]


def retained(folder, name="orientation_audit"):
    return json.loads((ROOT / ("tools/rhino_oracle/" + folder + "/" + name + ".json")).read_text())


def reversed_brep(record):
    result = copy.deepcopy(record)
    for face in result["topology"]["faces"]:
        face["reversed"] = not face["reversed"]
    return result


class OrientationTests(unittest.TestCase):
    def test_retained_capture_hashes(self):
        metadata = json.loads((ROOT / "docs/orientation-audit-provenance.json").read_text())
        for path, expected in metadata["retained_file_sha256"].items():
            self.assertEqual(hashlib.sha256((ROOT / path).read_bytes()).hexdigest(), expected)
        data = retained("observations")
        self.assertEqual(data["engine_version"], metadata["engine_version"])
        self.assertEqual(len(data["results"]), metadata["completed_operations"])

    def test_source_only_regeneration_and_bounded_validation(self):
        self.assertEqual(request(), retained("fixtures"))
        self.assertEqual(len(request()["operations"]), 36)
        self.assertEqual(len(compound_request()["operations"]), 20)
        self.assertEqual(len(face_request()["operations"]), 16)
        self.assertEqual(len(spatial_request()["operations"]), 24)
        for data in (request(), compound_request(), face_request(), spatial_request()):
            ids = [op["id"] for op in data["operations"]]
            self.assertEqual(len(ids), len(set(ids)))
            for op in data["operations"]: validate(op)

    def test_invalid_fields_are_rejected_before_launch(self):
        base = dict(op="orientation_audit", id="test", sources=[dict(kind="box")])
        for fields in (dict(id="x _Delete"), dict(flip=1), dict(preselect=None), dict(replace_flip=[]),
                       dict(insertion="macro"), dict(sources=[]), dict(sources=[{}] * 9),
                       dict(extra=True), dict(op="Flip"), dict(measure_volume=0)):
            with self.subTest(fields=fields), self.assertRaises(ValueError):
                validate(dict(base, **fields))
        for spec in (None, {}, dict(kind="unknown"), dict(kind="box", reversed=1),
                     dict(kind="box", size=True), dict(kind="box", size=3),
                     dict(kind="box", offset=101), dict(kind="box", offset=1.5),
                     dict(kind="box", translation=None), dict(kind="box", translation=[0, 0]),
                     dict(kind="box", translation=[0, 0, 101]), dict(kind="box", translation=[0, 0, True]),
                     dict(kind="box", translation=[0, 0, 0.5]), dict(kind="box", translation=[0, 0, "0"]),
                     dict(kind="box", translation=[0, 0, 0], offset=0),
                     dict(kind="box", parts=[]), dict(kind="point", reversed=True),
                     dict(kind="box", flipped_faces=[]), dict(kind="box", flipped_faces=[True]),
                     dict(kind="box", flipped_faces=[6]), dict(kind="box", flipped_faces=[0, 0]),
                     dict(kind="box", flipped_faces=[[]]), dict(kind="mesh", flipped_faces=[0]),
                     dict(kind="compound", parts=[None]),
                     dict(kind="compound", parts=[dict(kind="mesh")]),
                     dict(kind="compound", parts=[dict(kind="box")], offset=1),
                     dict(kind="compound", parts=[dict(kind="box")], translation=[0, 0, 0])):
            with self.subTest(spec=spec), self.assertRaises(ValueError):
                validate(dict(base, sources=[spec]))

    def test_mass_integration_is_optional_and_never_inferred_from_orientation(self):
        class Brep:
            IsSolid = True
            SolidOrientation = "Outward"
        for solid in (False, True):
            for measure in (False, True):
                for missing in (False, True):
                    with self.subTest(solid=solid, measure=measure, missing=missing):
                        source = Brep()
                        source.IsSolid = solid
                        mass = Mock(Volume=-56.)
                        compute = Mock(return_value=None if missing else mass)
                        record = Mock(return_value={"definitions": [1, 2, 3]})
                        rhino = SimpleNamespace(Geometry=SimpleNamespace(Brep=Brep,
                            VolumeMassProperties=SimpleNamespace(Compute=compute)))
                        with patch.dict("sys.modules", {"Rhino": rhino}):
                            value = geometry_record(source, {"_interchange_brep_record": record}, measure)
                        record.assert_called_once_with(source, include_samples=False)
                        self.assertEqual(value, dict(definitions=[1, 2, 3], type="brep", solid=solid,
                            orientation="Outward", volume=-56. if solid and measure and not missing else None))
                        if solid and measure:
                            compute.assert_called_once_with(source)
                        else:
                            compute.assert_not_called()
                        self.assertEqual(mass.Dispose.call_count, int(solid and measure and not missing))

    def test_mass_properties_are_disposed_when_volume_recording_fails(self):
        class Brep:
            IsSolid = True
            SolidOrientation = "Inward"
        mass = Mock(Volume=object())
        rhino = SimpleNamespace(Geometry=SimpleNamespace(Brep=Brep,
            VolumeMassProperties=SimpleNamespace(Compute=Mock(return_value=mass))))
        with patch.dict("sys.modules", {"Rhino": rhino}), self.assertRaises(TypeError):
            geometry_record(Brep(), {"_interchange_brep_record": Mock(return_value={})})
        mass.Dispose.assert_called_once_with()

    def test_flip_selection_events_identity_and_complete_definitions(self):
        cases = {op["id"]: op for op in request()["operations"]}
        count = 0
        for row in retained("observations")["results"]:
            op, value = cases[row["id"]], row["value"]
            command = value["command"]
            if command is None: continue
            count += 1
            self.assertTrue(command["succeeded"])
            endings = [e for e in command["events"] if e["name"] == "Flip"]
            self.assertEqual(len(endings), 1)
            self.assertEqual(endings[0]["result"], "Success")
            self.assertEqual(len(command["objects"]), len(op["sources"]))
            for i, (source, before, after) in enumerate(zip(op["sources"], value["inserted"], command["objects"])):
                kind = source["kind"]
                skipped = kind in ("box", "sphere", "point")
                self.assertEqual(after["selected"], op["preselect"] or skipped)
                self.assertEqual(after["name"], "source-%d" % i)
                self.assertEqual(after["groups"], 1)
                self.assertTrue(after["current_layer"])
                actual = after["geometry"]
                if skipped:
                    self.assertEqual(actual, before)
                elif kind in ("open_box", "plane"):
                    self.assertEqual(actual, reversed_brep(before))
                elif kind == "mesh":
                    expected = copy.deepcopy(before)
                    expected["faces"] = [[a, c, b] for a, b, c in before["faces"]]
                    self.assertEqual(actual, expected)
                elif kind == "line":
                    definition = actual["definition"]
                    original = before["definition"]
                    self.assertEqual(definition["control_points"], list(reversed(original["control_points"])))
                    self.assertEqual(definition["degree"], original["degree"])
                    for key in ("domain", "knots"):
                        self.assertEqual(definition[key], [-x for x in reversed(original[key])])
                else: self.fail("unexamined captured kind")
        self.assertEqual(count, 16)

    def test_single_shell_insertion_normalization_is_distinct_from_kernel_flip(self):
        cases = {op["id"]: op for op in request()["operations"]}
        for row in retained("observations")["results"]:
            op, value = cases[row["id"]], row["value"]
            if not op.get("replace_flip"): continue
            original, inserted = value["constructed"][0], value["inserted"][0]
            replaced = value["replacement"][0]["geometry"]
            if op["sources"][0]["kind"] in ("box", "sphere"):
                self.assertEqual(original["orientation"], "Inward" if op["sources"][0]["reversed"] else "Outward")
                self.assertEqual(inserted["orientation"], "Outward")
                self.assertEqual(replaced, inserted)
                expected = reversed_brep(original) if op["sources"][0]["reversed"] else copy.deepcopy(original)
                expected["orientation"] = "Outward"
                expected["volume"] = abs(expected["volume"])
                self.assertEqual(inserted, expected)
            else:
                self.assertEqual(inserted, original)
                if inserted["type"] == "brep":
                    self.assertEqual(replaced, reversed_brep(inserted))
                else:
                    expected = copy.deepcopy(inserted)
                    expected["faces"] = [[a, c, b] for a, b, c in inserted["faces"]]
                    self.assertEqual(replaced, expected)

    def test_mesh_orientation_diagnostic_does_not_override_raw_winding(self):
        # The getter retains +1 even though every face demonstrably reverses.
        # Keep the diagnostic verbatim; it is not a trustworthy reversal oracle.
        rows = {r["id"]: r["value"] for r in retained("observations")["results"]}
        value = rows["flip-mesh-True"]
        before, after = value["inserted"][0], value["command"]["objects"][0]["geometry"]
        self.assertEqual(before["orientation"], after["orientation"])
        self.assertNotEqual(before["faces"], after["faces"])


if __name__ == "__main__":
    unittest.main()
