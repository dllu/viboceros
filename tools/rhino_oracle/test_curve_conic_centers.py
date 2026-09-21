"""Conic recognition uses explicit tolerance, preserves input, and disposes ownership."""
from types import SimpleNamespace as NS
from unittest import TestCase
from unittest.mock import Mock, patch
from . import test_worker
import copy
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


class CurveConicCenterTests(TestCase):
    def setUp(self):
        test_worker.RhinoWorkerTests.setUp(self)
        conic = NS(IsValid=True, Center=NS(X=4., Y=-4., Z=7.))
        self.source = NS(Domain=NS(T0=1e12, T1=1e12+8), Dispose=Mock(),
                         TryGetCircle=Mock(return_value=(False, None)),
                         TryGetEllipse=Mock(return_value=(True, conic)))

    def execute(self):
        with patch.object(self.worker, "_nurbs_curve_from_definition", return_value=self.source), \
             patch.object(self.worker, "_measure", side_effect=lambda n, f: (f(), 42)):
            return self.worker._execute(dict(op="nurbs_curve_conic_centers", curve={}),
                                        1, {"absolute": 1e-9})

    def test_success_uses_native_parameters_full_center_and_explicit_tolerance(self):
        value, elapsed = self.execute()
        self.assertEqual(value, dict(circle_center=None, ellipse_center=[4., -4., 7.]))
        self.assertEqual(elapsed, 42)
        self.assertEqual((self.source.Domain.T0, self.source.Domain.T1), (1e12, 1e12+8))
        self.source.TryGetCircle.assert_called_once_with(1e-9)
        self.source.TryGetEllipse.assert_called_once_with(1e-9)
        self.source.Dispose.assert_called_once_with()

    def test_failed_recognition_is_null_not_a_default_center(self):
        self.source.TryGetEllipse.return_value = (False, NS(IsValid=False))
        value, _ = self.execute()
        self.assertEqual(value, dict(circle_center=None, ellipse_center=None))
        self.source.Dispose.assert_called_once_with()

    def test_errors_and_invalid_results_dispose_reference(self):
        for conic in [NS(IsValid=False), NS(IsValid=True, Center=NS(X=float("nan"), Y=0., Z=0.))]:
            self.source.Dispose.reset_mock()
            self.source.TryGetEllipse.return_value = (True, conic)
            with self.assertRaises(ValueError):
                self.execute()
            self.source.Dispose.assert_called_once_with()
        self.source.Dispose.reset_mock()
        self.source.TryGetEllipse.side_effect = RuntimeError("recognition failure")
        with self.assertRaisesRegex(RuntimeError, "recognition failure"):
            self.execute()
        self.source.Dispose.assert_called_once_with()

    def test_invalid_tolerance_is_rejected_before_constructing_a_curve(self):
        for absolute in [0., -1., float("nan"), float("inf")]:
            with patch.object(self.worker, "_nurbs_curve_from_definition") as build:
                with self.assertRaises(ValueError):
                    self.worker._curve_conic_centers(dict(curve={}), 1, dict(absolute=absolute))
                build.assert_not_called()


class ConicCenterEvidenceTests(TestCase):
    def load(self, name):
        return json.loads((ROOT / name).read_text())

    def test_generator_reproduces_both_portable_requests(self):
        from .references.conic_centers import request
        for mode, name in [("api", "conic_centers"), ("snaps", "elliptic_center_snaps")]:
            self.assertEqual(request(mode), self.load("tools/rhino_oracle/fixtures/%s.json" % name))

    def test_raw_hashes_and_engine_identity(self):
        provenance = self.load("docs/conic-center-provenance.json")
        for path, digest in provenance["retained_file_sha256"].items():
            self.assertEqual(hashlib.sha256((ROOT / path).read_bytes()).hexdigest(), digest)
        for name in ["conic_centers", "elliptic_center_snaps"]:
            record = self.load("tools/rhino_oracle/observations/%s.json" % name)
            self.assertEqual(record["engine_version"], provenance["engine_version"])
            self.assertIsNone(record.get("error"))

    def test_api_centers_keep_all_coordinates_and_explicit_differences(self):
        from .references.conic_centers import cases
        rows = self.load("tools/rhino_oracle/observations/conic_centers.json")["results"]
        self.assertEqual(len(rows), 32)
        nonconics = {"parabola", "hyperbola", "altered-span", "nonplanar", "near-conic"}
        for source, row in zip(cases(), rows):
            self.assertEqual(source["id"], row["id"])
            value = row["value"]
            if row["id"] != "circle": self.assertIsNone(value["circle_center"])
            if row["id"] in nonconics or row["id"] == "far-origin":
                self.assertIsNone(value["ellipse_center"])
            elif row["id"] in {"short-arc-0.01", "short-arc-0.001"}:
                self.assertGreater(abs(value["ellipse_center"][0]-4.), 1e-7)
            else:
                for a, b in zip(value["ellipse_center"], source["center"]):
                    self.assertLess(abs(a-b), 1e-9)

    def test_point_prompt_calibration_history_and_api_snap_disagreements(self):
        from . import split_edge_probe
        request = self.load("tools/rhino_oracle/fixtures/elliptic_center_snaps.json")
        rows = self.load("tools/rhino_oracle/observations/elliptic_center_snaps.json")["results"]
        api = {row["id"]: row["value"] for row in self.load("tools/rhino_oracle/observations/conic_centers.json")["results"]}
        self.assertEqual((len(request["operations"]), len(rows)), (62,62))
        matches = misses = gauges = short = 0
        for operation, row in zip(request["operations"], rows):
            with self.subTest(id=row["id"]):
                self.assertEqual(operation["id"], row["id"])
                prepared = copy.deepcopy(operation)
                prepared["sources"][0]["brep"]["artifact_path"] = "/owned/box.3dm"
                split_edge_probe.validate(prepared)
                value, pick = row["value"], operation["inputs"][0]["pick"]
                self.assertTrue(value["succeeded"] and value["history_tested"])
                self.assertEqual(value["before"], value["undo"])
                self.assertEqual(value["after"], value["redo"])
                self.assertEqual(value["before"][1:], value["after"][1:])
                self.assertEqual(len(value["pick_frames"]), 1)
                frame = value["pick_frames"][0]
                self.assertEqual(frame["aim"], pick["aim"])
                h = [sum(a*b for a,b in zip(r, pick["aim"]+[1.])) for r in frame["world_to_screen"]]
                for i in range(2):
                    self.assertLess(abs(h[i]/h[3]-frame["aim_client"][i]), 1e-7)
                    self.assertEqual(frame["click_client"][i], int(frame["aim_client"][i])+pick["offset"][i])
                before = value["before"][0]["geometry"]["brep"]["vertices"]
                after = value["after"][0]["geometry"]["brep"]["vertices"]
                self.assertEqual(after[:-1], before)
                self.assertEqual(after[-1][1:], [0.,0.])
                family = row["id"].removesuffix("-one-shot").removesuffix("-persistent")
                x = after[-1][0]
                if family in {"parabola", "hyperbola", "altered-span", "nonplanar", "near-conic"}:
                    self.assertGreater(abs(x-4.), 2.)
                    misses += 1
                elif family in {"positive-gauge", "negative-gauge", "large-gauge"}:
                    self.assertGreater(abs(x-4.), 2.)
                    self.assertAlmostEqual(api[family]["ellipse_center"][0], 4., places=9)
                    gauges += 1
                elif family in {"short-arc-0.01", "short-arc-0.001"}:
                    self.assertGreater(abs(x-4.), 1e-7)
                    short += 1
                else:
                    self.assertLess(abs(x-4.), 1e-9)
                    matches += 1
        self.assertEqual((matches, misses, gauges, short), (42,10,6,4))
