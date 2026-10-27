"""Source-bound global surface-distance evidence and explicit compatibility limits."""
import hashlib
import json
from pathlib import Path
import subprocess
import unittest
from .qualification_sources import with_shared_sources

ROOT = Path(__file__).resolve().parents[2]


class SurfaceDistanceBoundsTests(unittest.TestCase):
    def test_current_sources_cover_the_certificate_and_regressions(self):
        record = json.loads((ROOT / 'docs/surface-distance-validation.json').read_text())
        self.assertEqual(set(record['sha256']), set(with_shared_sources(list(record['sha256']))))
        for name, expected in record['sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT/name).read_bytes()).hexdigest(), expected, name)
        for name in ['crates/viboceros-geometry/src/surface_distance.rs',
                     'crates/viboceros-geometry/src/surface_pullback/certificate/surface_minimum.rs',
                     'crates/viboceros-geometry/src/surface_pullback/certificate/surface_minimum/patch.rs',
                     'crates/viboceros-geometry/src/surface_pullback/certificate/surface/tests.rs',
                     'crates/viboceros-geometry/tests/surface_minimum.rs',
                     'crates/viboceros-geometry/tests/curve_minimum_precision.rs',
                     'crates/viboceros-geometry/tests/sweep_surface_distance.rs']:
            self.assertIn(name, record['sha256'])

    def test_distance_scope_does_not_claim_surface_or_trim_equivalence(self):
        record = json.loads((ROOT / 'docs/surface-distance-validation.json').read_text())
        self.assertTrue(record['implemented'])
        self.assertFalse(record['trimmed_faces_supported'])
        self.assertFalse(record['complete_rhino_sweep_compatibility'])
        self.assertEqual(record['limits']['degree'], 16)
        self.assertEqual(record['limits']['controls'], 4096)
        self.assertEqual(record['limits']['depth'], 48)
        self.assertEqual(record['limits']['pending_patches'], 65536)
        self.assertEqual(record['regressions']['analytic_surface_tests'], 8)
        witness = record['spatial_sweep_witness']
        self.assertFalse(witness['complete_surface_agreement_qualified'])
        self.assertLessEqual(witness['maximum_error'], 1e-9)
        self.assertEqual(witness['source_membership_error'], 1e-12)
        self.assertIn(witness['classification'], ['proved_gap', 'witness_within_epsilon', 'inconclusive'])
        if witness['classification'] == 'proved_gap':
            self.assertGreater(witness['image_distance_lower_bound'], witness['comparison_epsilon'])

    def test_old_precision_failure_remains_tied_to_its_original_code(self):
        record = json.loads((ROOT / 'docs/surface-distance-validation.json').read_text())
        before = record['precision_regression_before_fix']
        self.assertEqual(before['exit_code'], 101)
        self.assertGreater(before['upper_distance'], 1.)
        self.assertEqual(before['maximum_error'], float.fromhex('0x0.0000000000001p-1022'))
        for identity, expected in record['precision_before_fix_git_sha256'].items():
            original = subprocess.check_output(['git', 'show', identity], cwd=ROOT)
            self.assertEqual(hashlib.sha256(original).hexdigest(), expected, identity)


if __name__ == '__main__':
    unittest.main()
