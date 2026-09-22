"""Shared-source document admission; full raw observations are never rewritten."""
import copy
import hashlib
import json
from fractions import Fraction as F
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

from .document_brep_probe import validate, geometry_record, run
from .references.document_brep import request

ROOT = Path(__file__).resolve().parents[2]


class DocumentBrepTests(unittest.TestCase):
    def test_source_only_matrix_is_bounded_and_reproducible(self):
        retained = json.loads((ROOT / "tools/rhino_oracle/fixtures/document_brep.json").read_text())
        self.assertEqual(request(), retained)
        self.assertEqual(len(retained["operations"]), 64)
        self.assertEqual(len({op["id"] for op in retained["operations"]}), 64)
        for op in retained["operations"]: validate(op)

    def test_invalid_host_fields_are_rejected(self):
        base = request()["operations"][0]
        for fields in (dict(op="unknown"), dict(id="x _Delete"), dict(id=1), dict(extra=True),
                       dict(sources=[]), dict(sources=[{}]*9), dict(sources=[None]), dict(selected=1),
                       dict(selected=None), dict(insertion="script"), dict(flip_faces=[-1]),
                       dict(flip_faces=[True]), dict(flip_faces=[[]]), dict(flip_faces=[0,0]),
                       dict(artifact_path=1), dict(artifact_path="")):
            with self.subTest(fields=fields), self.assertRaises(ValueError):
                validate(dict(base, **fields))
        for iterations in (0, 2, True, 1.0, None):
            with self.subTest(iterations=iterations), self.assertRaises(ValueError):
                validate(base, iterations)

    def test_full_capture_follows_whole_object_sense_without_changing_any_other_definition(self):
        data = json.loads((ROOT / "tools/rhino_oracle/observations/document_brep.json").read_text())
        self.assertEqual(len(data["results"]), 64)
        solid, opened = 0, 0
        for op, row in zip(request()["operations"], data["results"]):
            self.assertEqual(row["id"], op["id"])
            value = row["value"]
            for key in ("source_unchanged", "replacement_unchanged", "replaced"):
                self.assertTrue(value[key])
            reverse = copy.deepcopy(value["input"]["geometry"])
            for face in reverse["topology"]["faces"]: face["reversed"] = not face["reversed"]
            self.assertEqual(value["replacement_input"]["geometry"], reverse)
            for source_key, result_key in (("input", "inserted"), ("replacement_input", "replacement")):
                expected = copy.deepcopy(value[source_key])
                if expected["orientation"] == "Inward":
                    for face in expected["geometry"]["topology"]["faces"]:
                        face["reversed"] = not face["reversed"]
                    expected["orientation"] = "Outward"
                state = value[result_key]
                self.assertEqual(state["geometry"], expected)
                self.assertEqual(state["selected"], op["selected"])
                self.assertEqual(state["name"], "Source")
                self.assertEqual(state["group_count"], 1)
                self.assertEqual(state["object_count"], 1)
                self.assertTrue(state["identity_preserved"])
                self.assertTrue(state["current_layer"])
            solid += value["input"]["solid"]
            opened += not value["input"]["solid"]
        self.assertEqual((solid, opened), (48, 16))

    def test_box_normals_and_signed_volume_witnesses_are_independent_of_orientation_getters(self):
        rows = json.loads((ROOT / "tools/rhino_oracle/observations/document_brep.json").read_text())["results"]
        for op, row in zip(request()["operations"], rows):
            if op["id"].startswith("corner-"): continue
            geometry = row["value"]["input"]["geometry"]
            start = 0
            volume = F(0)
            for source in op["sources"]:
                box = source["source"]
                center = [(F(a)+F(b))/2 for a,b in zip(box["min"], box["max"])]
                volume += (-1 if source["reversed"] else 1) * F(box["max"][0]-box["min"][0])**3
                count = len(box.get("keep_faces", range(6)))
                for index in range(start, start+count):
                    definition = geometry["faces"][index]["definition"]
                    self.assertEqual(definition["degree"], [1,1])
                    self.assertEqual(definition["control_count"], [2,2])
                    self.assertTrue(all(cp["weight"] == 1 for cp in definition["control_points"]))
                    p = [[F(x) for x in cp["point"]] for cp in definition["control_points"]]
                    self.assertEqual(p[3], [p[1][i]+p[2][i]-p[0][i] for i in range(3)])
                    u, v = [[p[k][i]-p[0][i] for i in range(3)] for k in (1,2)]
                    n = [u[1]*v[2]-u[2]*v[1], u[2]*v[0]-u[0]*v[2], u[0]*v[1]-u[1]*v[0]]
                    dot = sum(n[i]*(sum(pt[i] for pt in p)/4-center[i]) for i in range(3))
                    if geometry["topology"]["faces"][index]["reversed"]: dot = -dot
                    self.assertNotEqual(dot, 0)
                    self.assertEqual(dot < 0, source["reversed"] ^ (index in op["flip_faces"]))
                start += count
            self.assertEqual(start, len(geometry["faces"]))
            if op["id"].startswith("negative-volume-"):
                self.assertEqual(abs(volume), 56)
                self.assertEqual(volume < 0, row["value"]["input"]["orientation"] == "Outward")
            if op["id"].startswith(("zero-volume-", "coincident-")):
                self.assertEqual(volume, 0)

    def test_retained_provenance_and_unfiltered_baseline_report(self):
        metadata = json.loads((ROOT / "docs/document-brep-provenance.json").read_text())
        for path, digest in metadata["retained_file_sha256"].items():
            self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(), digest)
        report = json.loads((ROOT / "tools/rhino_oracle/observations/document_brep_before_report.json").read_text())
        self.assertEqual((report["cases"], report["matched"], report["mismatched"], report["native_failed"]), (64,16,48,0))
        self.assertEqual(len(report["operations"]), 64)
        for row in report["operations"]:
            self.assertIsNone(row["native_error"])
            self.assertEqual(row["comparison"]["max_absolute_error"], 0)
            self.assertEqual(row["passed"], row["id"].startswith(("open-", "inconsistent-")))

    def test_definition_only_geometry_never_samples_or_measures_volume(self):
        record = Mock(return_value={"topology": {"faces": []}})
        rhino = SimpleNamespace(Geometry=SimpleNamespace(EdgeAdjacency=SimpleNamespace(Interior=2)))
        brep = SimpleNamespace(SolidOrientation="Inward", IsSolid=True,
            Edges=[SimpleNamespace(Valence=2)])
        with patch.dict("sys.modules", {"Rhino": rhino}):
            result = geometry_record(brep, {"_interchange_brep_record": record})
        record.assert_called_once_with(brep, include_samples=False)
        self.assertEqual(result, dict(orientation="Inward", solid=True, closed=True,
            geometry={"topology": {"faces": []}}))

    def test_repeated_operations_have_distinct_owned_paths_and_do_not_mutate_callers(self):
        from .client import _owned_artifact_request
        op = dict(request()["operations"][0], artifact_path="/unowned/model.3dm")
        source = dict(operations=[op, op])
        before = copy.deepcopy(source)
        with _owned_artifact_request(source) as prepared:
            paths = [Path(o["artifact_path"]) for o in prepared["operations"]]
            self.assertNotEqual(paths[0], paths[1])
            self.assertTrue(all(p.parent.is_dir() and not p.exists() for p in paths))
            self.assertTrue(all(p != Path("/unowned/model.3dm") for p in paths))
        self.assertTrue(all(not p.parent.exists() for p in paths))
        self.assertEqual(source, before)

    def test_adapter_uses_raw_replacement_and_cleans_only_owned_objects_on_failure(self):
        # Deliberately make insertion change sense. A replacement derived from
        # the inserted object would then be the wrong shared input.
        for failure in (None, "add", "replace", "record"):
            with self.subTest(failure=failure):
                owned = []
                class Brep:
                    IsValid = IsSolid = True
                    Edges = [SimpleNamespace(Valence=2)]
                    def __init__(self, inward):
                        self.inward = inward
                        self.disposed = False
                    @property
                    def SolidOrientation(self): return "Inward" if self.inward else "Outward"
                    def DuplicateBrep(self):
                        result = Brep(self.inward)
                        owned.append(result)
                        return result
                    def Flip(self): self.inward = not self.inward
                    def Dispose(self): self.disposed = True
                source = Brep(True)
                attrs = SimpleNamespace(Name=None, LayerIndex=0, GroupCount=0, Dispose=Mock())
                original = SimpleNamespace(Id=7, IsSelected=lambda _: True)
                objects = {7: original}
                deleted, selected = [], {7}
                def add(brep, attributes):
                    if failure == "add": return 0
                    self.assertIs(brep, source)
                    objects[9] = SimpleNamespace(Id=9, Geometry=Brep(False), Attributes=attributes,
                        IsSelected=lambda _: 9 in selected)
                    return 9
                def replace(key, brep):
                    self.assertEqual(key, 9)
                    self.assertFalse(brep.inward)
                    self.assertTrue(source.inward)
                    if failure == "replace": raise RuntimeError("replacement failed")
                    objects[key].Geometry = Brep(brep.inward)
                    return True
                def delete(key, quiet):
                    self.assertTrue(quiet)
                    deleted.append(key)
                    del objects[key]
                def group_add(name, keys):
                    self.assertEqual(keys, [9])
                    attrs.GroupCount = 1
                    return 2
                table = SimpleNamespace(GetObjectList=lambda _: list(objects.values()), Add=add,
                    FindId=lambda key: objects.get(key), Replace=replace, Delete=delete,
                    UnselectAll=selected.clear, Select=selected.add)
                groups = SimpleNamespace(Add=group_add, Delete=Mock())
                doc = SimpleNamespace(Objects=table, Groups=groups,
                    Layers=SimpleNamespace(CurrentLayerIndex=0))
                model = SimpleNamespace(Objects=[SimpleNamespace(Geometry=source)], Dispose=Mock())
                rhino = SimpleNamespace(Geometry=SimpleNamespace(Brep=Brep,
                    EdgeAdjacency=SimpleNamespace(Interior=2)), FileIO=SimpleNamespace(
                    File3dm=SimpleNamespace(Read=Mock(return_value=model))),
                    RhinoDoc=SimpleNamespace(ActiveDoc=doc), DocObjects=SimpleNamespace(
                    ObjectEnumeratorSettings=SimpleNamespace, ObjectAttributes=lambda: attrs))
                system = SimpleNamespace(Guid=SimpleNamespace(Empty=0, NewGuid=lambda: 1))
                def record(brep, include_samples):
                    self.assertFalse(include_samples)
                    if failure == "record" and 9 in objects: raise RuntimeError("record failed")
                    return dict(inward=brep.inward)
                operation = dict(request()["operations"][0], artifact_path="/owned/source.3dm", selected=True)
                with patch.dict("sys.modules", {"Rhino": rhino, "System": system}):
                    if failure:
                        with self.assertRaises((ValueError, RuntimeError)):
                            run(operation, 1, {"_interchange_brep_record": record})
                    else:
                        result, elapsed = run(operation, 1, {"_interchange_brep_record": record})
                        self.assertEqual(elapsed, 0)
                        self.assertTrue(result["source_unchanged"])
                        self.assertTrue(result["replacement_unchanged"])
                        self.assertTrue(result["replacement"]["selected"])
                        self.assertTrue(result["replaced"])
                        self.assertEqual(result["input"]["orientation"], "Inward")
                        self.assertEqual(result["replacement_input"]["orientation"], "Outward")
                self.assertEqual(objects, {7: original})
                self.assertEqual(selected, {7})
                self.assertEqual(deleted, [] if failure == "add" else [9])
                self.assertEqual(groups.Delete.call_count, 0 if failure == "add" else 1)
                self.assertTrue(all(brep.disposed for brep in owned))
                model.Dispose.assert_called_once_with()
                attrs.Dispose.assert_called_once_with()


if __name__ == "__main__":
    unittest.main()
