"""Guard the transfer audit against accepting geometry or state changes."""
import copy
import json
from pathlib import Path
import unittest

from .brep_transfer_comparison import comparison, read

ROOT = Path(__file__).resolve().parents[2]


class BrepTransferComparisonTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.before = json.loads(read(ROOT / "docs/join-pick-surfaces-reference.json.gz"))
        cls.after = json.loads(read(ROOT / "docs/native-pole-join-surfaces-candidate.json.gz"))

    def pair(self):
        def one(response):
            result = copy.deepcopy(response)
            result["outcomes"] = [next(o for o in result["outcomes"]
                                      if o["result"]["id"] == "six-inward-Join-pre")]
            return result
        return one(self.before), one(self.after)

    def assert_rejected(self, before, after):
        report = comparison(before, after)
        self.assertEqual(report["raw_value_matches"] + report["cyclic_trim_start_only"], 0)

    def test_retained_native_cases_preserve_all_nontrim_fields(self):
        report = comparison(self.before, self.after)
        self.assertEqual(report["cases"], 66)
        self.assertEqual(report["raw_value_matches"], 58)
        self.assertEqual(report["cyclic_trim_start_only"], 8)
        self.assertEqual(report["outside_trim_values_exact"], 66)

    def test_a_changed_spatial_vertex_is_rejected(self):
        before, after = self.pair()
        after["outcomes"][0]["result"]["value"]["objects"][0]["brep"]["vertices"][0][0] += 1
        self.assert_rejected(before, after)

    def test_changed_selection_is_rejected(self):
        before, after = self.pair()
        obj = after["outcomes"][0]["result"]["value"]["objects"][0]
        obj["selected"] = not obj["selected"]
        self.assert_rejected(before, after)

    def test_trim_parameterization_is_retained_even_for_the_same_curve_image(self):
        before, after = self.pair()
        curve = after["outcomes"][0]["result"]["value"]["objects"][0]["brep"]["trim_curves"][0][0][0]
        curve["domain"] = [2 * k for k in curve["domain"]]
        curve["knots"] = [2 * k for k in curve["knots"]]
        self.assert_rejected(before, after)

    def test_audit_failures_cannot_be_counted_as_equivalent_geometry(self):
        before, after = self.pair()
        after["outcomes"][0] = {"status": "error", "error": "native operation failed"}
        with self.assertRaises(ValueError):
            comparison(before, after)


if __name__ == "__main__":
    unittest.main()
