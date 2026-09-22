"""Stationary trim correspondence on identical, unmodified shared B-reps."""
import copy
import hashlib
import json
from pathlib import Path
import unittest

from .references.stationary_trims import request

ROOT = Path(__file__).resolve().parents[2]
OBS = ROOT / "tools/rhino_oracle/observations"


class StationaryTrimTests(unittest.TestCase):
    def test_source_regeneration_and_retained_hashes(self):
        fixture = json.loads((ROOT / "tools/rhino_oracle/fixtures/stationary_trims.json").read_text())
        self.assertEqual(fixture, request())
        provenance = json.loads((ROOT / "docs/trim-correspondence-provenance.json").read_text())
        for path, expected in provenance["retained_file_sha256"].items():
            self.assertEqual(hashlib.sha256((ROOT / path).read_bytes()).hexdigest(), expected, path)

    def test_complete_observations_retain_encoding_and_analytic_orientation(self):
        observed = json.loads((OBS / "stationary_trims.json").read_text())
        self.assertEqual(observed["engine_version"], "8.32.26160.13001")
        self.assertEqual(len(observed["results"]), 18)
        unencoded = {}
        for operation, row in zip(request()["operations"], observed["results"]):
            self.assertEqual(row["id"], operation["id"])
            source = operation["sources"][0]
            value = row["value"]
            self.assertEqual(value["orientation"], "Inward" if source["reversed"] else "Outward")
            self.assertTrue(value["solid"])
            self.assertTrue(value["closed"])
            encoding = source["trim_endpoint_encoding"]
            geometry = copy.deepcopy(value["geometry"])
            for face in geometry["faces"]:
                for boundary in face["loops"]:
                    for trim in boundary:
                        definition = trim.pop("definition")
                        self.assertEqual(definition["degree"], encoding["degree"])
                        self.assertEqual(definition["knots"], encoding["knots"])
                        controls = definition["control_points"]
                        endpoints = (controls[0]["point"], controls[-1]["point"])
                        self.assertEqual([p["weight"] for p in controls], encoding["weights"])
                        self.assertEqual([p["point"] for p in controls],
                                         [endpoints[i] for i in encoding["endpoint_indices"]])
            key = (source["source"]["type"], source["reversed"])
            self.assertEqual(geometry, unencoded.setdefault(key, geometry))

    def test_constructor_failures_are_retained_not_converted_to_unknown(self):
        before = json.loads((OBS / "stationary_trims_native_before.json").read_text())
        failures = [o for o in before["outcomes"] if o["status"] == "failure"]
        self.assertEqual(len(failures), 6)
        self.assertEqual({o["id"] for o in failures},
                         {o["id"] for o in request()["operations"] if "stationary-end" in o["id"]})
        for failure in failures:
            self.assertEqual(failure["error"]["kind"], "geometry")
            self.assertIn("edge interior leaves its lifted p-curve", failure["error"]["message"])
        observed = {r["id"]: r["value"] for r in json.loads((OBS / "stationary_trims.json").read_text())["results"]}
        for outcome in before["outcomes"]:
            if outcome["status"] == "success":
                row = outcome["result"]
                self.assertEqual(row["value"], observed[row["id"]])
        for when, matches, errors in (("before", 12, 6), ("after", 18, 0)):
            report = json.loads((OBS / ("stationary_trims_%s_report.json" % when)).read_text())
            self.assertEqual((report["cases"], report["matched"], report["native_failed"], report["mismatched"]),
                             (18, matches, errors, 0))
            for operation in report["operations"]:
                if operation["comparison"] is not None:
                    self.assertEqual(operation["comparison"]["max_absolute_error"], 0)
                    self.assertEqual(operation["comparison"]["differences"], [])
