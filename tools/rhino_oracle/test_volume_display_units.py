"""Display conversion witnesses, strict captures, and input/target separation."""
import copy
import hashlib
import json
from pathlib import Path
import unittest

from .area_centroid_probe import validate
from .client import OracleProtocolError
from .references.volume_display_units import request
from .references.volume_display_unit_integrals import exact_volume, reference
from .volume_command_replay import history_volume, prepare

ROOT = Path(__file__).resolve().parents[2]


def inputs():
    return [json.loads((ROOT / ("tools/rhino_oracle/" + folder + "/volume_display_units.json")).read_text())
            for folder in ("fixtures", "observations")]


class VolumeDisplayUnitTests(unittest.TestCase):
    def test_capture_and_comparison_hashes_are_preserved(self):
        metadata = json.loads((ROOT / "docs/volume-display-units-provenance.json").read_text())
        for path, digest in metadata["retained_file_sha256"].items():
            self.assertEqual(hashlib.sha256((ROOT / path).read_bytes()).hexdigest(), digest)
        _, observed = inputs()
        self.assertEqual(observed["engine_version"], metadata["engine_version"])
        self.assertEqual(sum(row["value"]["succeeded"] for row in observed["results"]),
                         metadata["successful_measurements"])
        report = json.loads((ROOT / "docs/volume-display-units-comparison.json").read_text())
        self.assertTrue(report["passed"])
        self.assertEqual(sum(row["passed"] for row in report["operations"]), metadata["native_command_matches"])
        self.assertEqual(max(row["max_absolute_error"] for row in report["operations"]),
                         metadata["maximum_absolute_difference"])

    def test_source_regeneration_and_all_thirty_captured_command_values(self):
        data, observed = inputs()
        self.assertEqual(data, request())
        self.assertEqual(json.loads((ROOT / "tools/rhino_oracle/fixtures/volume_display_units_reference.json").read_text()), reference())
        self.assertEqual(len(data["operations"]), 30)
        native, evidence = prepare(data, observed)
        self.assertEqual(native, data)
        for op, row in zip(data["operations"], evidence["results"]):
            self.assertEqual(row["value"]["unit_setup"], [False] * len(op["unit_setup"]))
            if op.get("open_confirmation") == "no":
                self.assertFalse(row["value"]["succeeded"])
                self.assertIsNone(row["value"]["volume"])
            else:
                self.assertTrue(row["value"]["succeeded"])
                self.assertLessEqual(abs(row["value"]["volume"] - float(exact_volume(op))), 1e-9, op["id"])

    def test_observed_values_and_api_diagnostics_do_not_change_native_input(self):
        data, observed = inputs()
        changed = copy.deepcopy(observed)
        row = changed["results"][0]["value"]
        row["history"] = row["history"].replace("Volume = 1 (", "Volume = 123 (")
        row["properties"][0]["volume"] = 987.
        native, evidence = prepare(data, changed)
        self.assertEqual(native, data)
        self.assertEqual(evidence["results"][0]["value"]["volume"], 123.)

    def test_wrong_labels_and_incomplete_or_successful_setup_are_rejected(self):
        data, observed = inputs()
        for mutate in (
            lambda v: v.update(history=v["history"].replace("cubic meters", "cubic feet")),
            lambda v: v.update(unit_setup=[]),
            lambda v: v["unit_setup"][0].update(succeeded=True),
            lambda v: v["unit_setup"][0].update(units="Foot"),
            lambda v: v["unit_setup"][0].update(history="missing prompt"),
            lambda v: v.update(display=dict(precision=7, units="Inches")),
        ):
            changed = copy.deepcopy(observed)
            mutate(changed["results"][0]["value"])
            with self.assertRaises(OracleProtocolError):
                prepare(data, changed)
        with self.assertRaises(OracleProtocolError):
            history_volume(observed["results"][0]["value"])

    def test_option_schema_is_bounded_and_not_an_arbitrary_macro(self):
        op = request()["operations"][0]
        for mutation in (dict(display_units="Meter _Delete"), dict(preselect=True),
                         dict(model_units=True), dict(model_units=255), dict(unit_setup=[]),
                         dict(unit_setup=["Meter"] * 5), dict(unit_setup=[None]),
                         dict(op="volume_centroid_command")):
            with self.assertRaises(ValueError):
                validate(dict(op, **mutation), "volume", False)


if __name__ == "__main__":
    unittest.main()
