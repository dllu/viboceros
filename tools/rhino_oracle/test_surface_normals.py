"""Public NormalAt records are independent of the sampled derivative cross."""
import hashlib
import json
import math
from pathlib import Path
import subprocess
import sys
import unittest

from .references.surface_normals import request

ROOT = Path(__file__).resolve().parents[2]


class SurfaceNormalTests(unittest.TestCase):
    def test_source_regeneration_capture_hashes_and_analytic_plane_directions(self):
        fixture = json.loads((ROOT / "tools/rhino_oracle/fixtures/surface_normals.json").read_text())
        self.assertEqual(fixture, request())
        provenance = json.loads((ROOT / "docs/surface-normal-provenance.json").read_text())
        for path, expected in provenance["retained_file_sha256"].items():
            self.assertEqual(hashlib.sha256((ROOT / path).read_bytes()).hexdigest(), expected)
        data = json.loads((ROOT / "tools/rhino_oracle/observations/surface_normals.json").read_text())
        self.assertEqual(data["engine_version"], provenance["engine_version"])
        self.assertEqual(len(data["results"]), 20)
        self.assertEqual([r["id"] for r in data["results"]], [op["id"] for op in fixture["operations"]])
        former_failures = 0
        for i, row in enumerate(data["results"]):
            value = row["value"]
            if i < 18:
                sign = -1 if i % 2 else 1
                expected = [sign*x/math.sqrt(6.) for x in (-1., -2., 1.)]
                for actual, expected in zip(value["normal"], expected):
                    self.assertLessEqual(abs(actual-expected), 1e-14)
                a, b = value["derivative_u"], value["derivative_v"]
                cross = [a[(j+1)%3]*b[(j+2)%3]-a[(j+2)%3]*b[(j+1)%3] for j in range(3)]
                former_failures += math.hypot(*cross) <= 1e-9
            self.assertLess(abs(math.hypot(*value["normal"])-1.), 1e-14)
        self.assertGreater(former_failures, 0)

    def test_fraction_bernstein_normal_reference_is_regenerated_without_native_outputs(self):
        result = subprocess.run([sys.executable, "tools/numerics/generate_surface_normal_reference.py"],
                                cwd=ROOT, check=True, capture_output=True, timeout=30)
        expected = (ROOT / "crates/viboceros-geometry/src/nurbs_surface/evaluate/normal/reference.txt").read_bytes()
        self.assertEqual(result.stdout, expected)


if __name__ == "__main__":
    unittest.main()
