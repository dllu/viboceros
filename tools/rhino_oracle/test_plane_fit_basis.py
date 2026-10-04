"""Retained independent SVD diagnostics are not native orientation proofs."""
import hashlib
import json
import math
from pathlib import Path
from unittest import TestCase

ROOT = Path(__file__).resolve().parents[2]


class PlaneFitBasisTests(TestCase):
    def test_complete_native_plane_diagnostic_coverage_without_an_orientation_match(self):
        data = json.loads((ROOT / "docs/plane-fit-basis-diagnostics.json").read_text())
        native = []
        for name in ("circle_fit_diagnostics", "circle_fit_distant_arcs", "circle_fit_distant_noisy"):
            fixture = json.loads((ROOT / "tools/rhino_oracle/fixtures" / (name + ".json")).read_text())
            results = json.loads((ROOT / "tools/rhino_oracle/observations" / (name + ".json")).read_text())
            native.extend(zip(fixture["operations"], results["results"]))
        self.assertEqual(data["native_records"], 44)
        self.assertEqual(len(data["measurements"]), len(native))
        self.assertEqual(data["centroid"], "sequential arithmetic mean")
        self.assertEqual(data["input_scaling"], "none")
        variants = {solver + "_" + centered + "_" + width + cross
                    for solver in ("faer", "nalgebra") for centered in ("false", "true")
                    for width, cross in (("3", ""), ("3", "_cross"), ("4", ""))}
        matching = dict.fromkeys(variants, 0)
        for (op, row), measurement in zip(native, data["measurements"]):
            self.assertEqual(measurement["id"], row["id"])
            expected = variants - ({"nalgebra_false_4", "nalgebra_true_4"} if len(op["points"]) == 3 else set())
            self.assertEqual(set(measurement["variants"]), expected)
            plane = row["value"]["plane"]
            # The public fitted plane passes through the centroid but stores the
            # nearest point on that plane to world zero as its origin.
            center = [math.fsum(p[i] for p in op["points"]) / len(op["points"]) for i in range(3)]
            offset = sum(a*b for a, b in zip(plane["origin"], plane["normal"]))
            self.assertLessEqual(math.dist(plane["origin"], [offset*n for n in plane["normal"]]), 1e-12)
            self.assertLessEqual(abs(sum((a-b)*n for a, b, n in zip(center, plane["origin"], plane["normal"]))), 1e-12)
            for key, value in measurement["variants"].items():
                if value is None:
                    continue
                self.assertLessEqual(abs(math.hypot(*value["normal"])-1.), 1e-12)
                self.assertAlmostEqual(value["normal_error"], math.dist(value["normal"], plane["normal"]), places=14)
                self.assertTrue(math.isfinite(value["basis_error"]))
                self.assertGreaterEqual(value["basis_error"], 0.)
                if value["normal_error"] <= data["normal_epsilon"] and value["basis_error"] <= data["basis_epsilon"]:
                    matching[key] += 1
        # These experiments must remain clearly separate from a compatible fitter.
        self.assertTrue(all(count < 40 for count in matching.values()))

    def test_diagnostic_provenance_hashes(self):
        record = json.loads((ROOT / "docs/plane-fit-basis-provenance.json").read_text())
        self.assertTrue(record["private_xvfb_native_inputs"])
        self.assertFalse(record["full_native_orientation_parity"])
        self.assertEqual(record["native_records"], 44)
        self.assertEqual(len(record["variants"]), 12)
        for path, expected in record["sha256"].items():
            self.assertEqual(hashlib.sha256((ROOT / path).read_bytes()).hexdigest(), expected, path)
