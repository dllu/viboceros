"""File admission is measured independently of Add/Replace and lossless readers."""
import copy
import hashlib
import json
from contextlib import nullcontext
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

from .document_brep_probe import run_import, validate
from .references.document_brep import import_request

ROOT=Path(__file__).resolve().parents[2]


def load(name):
    return json.loads((ROOT / ("tools/rhino_oracle/"+name)).read_text())


class DocumentBrepImportTests(unittest.TestCase):
    def test_source_matrix_and_owned_paths(self):
        from .client import _owned_artifact_request
        data=import_request()
        self.assertEqual(data,load("fixtures/document_brep_import.json"))
        self.assertEqual(len(data["operations"]),16)
        for op in data["operations"]:
            validate(op)
            for invalid in (dict(selected=True),dict(insertion="generic")):
                with self.assertRaises(ValueError): validate(dict(op,**invalid))
        before=copy.deepcopy(data)
        with _owned_artifact_request(data) as prepared:
            paths=[Path(op["artifact_path"]) for op in prepared["operations"]]
            self.assertEqual(len(set(paths)),16)
            self.assertTrue(all(p.parent.is_dir() and not p.exists() for p in paths))
        self.assertEqual(data,before)
        self.assertTrue(all(not p.parent.exists() for p in paths))

    def test_import_globally_normalizes_inward_inputs_and_preserves_all_other_definitions(self):
        observed=load("observations/document_brep_import.json")
        earlier={r["id"]:r["value"] for r in load("observations/document_brep.json")["results"]}
        rows=observed["results"]
        self.assertEqual(len(rows),16)
        flipped=0
        for op,row in zip(import_request()["operations"],rows):
            self.assertEqual(row["id"],op["id"])
            value=row["value"]
            old=earlier[op["id"].removesuffix("-import")+"-generic-selected-False"]
            self.assertEqual(value["input"],old["input"])
            self.assertEqual(value["imported"],old["inserted"]["geometry"])
            expected=copy.deepcopy(value["input"])
            if expected["orientation"]=="Inward":
                flipped+=1
                expected["orientation"]="Outward"
                for face in expected["geometry"]["topology"]["faces"]: face["reversed"]=not face["reversed"]
            self.assertEqual(value["imported"],expected)
            self.assertTrue(value["source_unchanged"])
            self.assertTrue(value["succeeded"])
            self.assertEqual(value["object_count"],1)
        self.assertEqual(flipped,6)

    def test_headless_import_is_disposed_on_success_and_failure_without_active_document_access(self):
        class Brep:
            IsValid=True
        for failed in (False,True):
            source=Brep()
            model=SimpleNamespace(Objects=[SimpleNamespace(Geometry=source)],Dispose=Mock())
            imported=Brep()
            document=SimpleNamespace(Import=Mock(return_value=not failed),Dispose=Mock(),
                Objects=SimpleNamespace(GetObjectList=Mock(return_value=[SimpleNamespace(Geometry=imported)])))
            rhino=SimpleNamespace(Geometry=SimpleNamespace(Brep=Brep),
                FileIO=SimpleNamespace(File3dm=SimpleNamespace(Read=Mock(return_value=model))),
                RhinoDoc=SimpleNamespace(CreateHeadless=Mock(return_value=document)),
                UnitSystem=SimpleNamespace(Millimeters=2),
                DocObjects=SimpleNamespace(ObjectEnumeratorSettings=SimpleNamespace))
            host={"_document_tolerance":Mock(return_value=nullcontext())}
            op=dict(import_request()["operations"][0],artifact_path="/owned/model.3dm")
            with patch.dict("sys.modules",{"Rhino":rhino}), patch(
                    "tools.rhino_oracle.document_brep_probe.geometry_record",return_value={"geometry":"raw"}):
                if failed:
                    with self.assertRaises(ValueError): run_import(op,1,{},host)
                else:
                    result,elapsed=run_import(op,1,{},host)
                    self.assertEqual(elapsed,0)
                    self.assertEqual(result["input"],result["imported"])
                    self.assertTrue(result["source_unchanged"])
            document.Import.assert_called_once_with("Z:\\owned\\model.3dm")
            rhino.RhinoDoc.CreateHeadless.assert_called_once_with(None)
            document.Dispose.assert_called_once_with()
            model.Dispose.assert_called_once_with()
            host["_document_tolerance"].assert_called_once_with(document,{})

    def test_baseline_and_after_reports_keep_coincident_discrepancies_explicit(self):
        metadata=json.loads((ROOT/"docs/document-normalization-provenance.json").read_text())
        for path,sha in metadata["retained_file_sha256"].items():
            self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(),sha)
        for name,matches,gaps in [("document_brep_import_before_report",9,7),
                                   ("document_brep_import_after_report",14,2),
                                   ("document_brep_after_report",56,8)]:
            report=load("observations/"+name+".json")
            self.assertEqual((report["matched"],report["mismatched"],report["native_failed"]),(matches,gaps,0))
            self.assertEqual(len(report["operations"]),matches+gaps)
            for row in report["operations"]:
                self.assertIsNone(row["native_error"])
                self.assertEqual(row["comparison"]["max_absolute_error"],0)
                if "after" in name: self.assertEqual(row["passed"],not row["id"].startswith("coincident-"))
