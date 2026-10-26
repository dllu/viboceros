"""Retain scale-dependent joined bases without calling raw pieces native parity."""
import gzip
import hashlib
import json
from pathlib import Path
import unittest
from .generate_loft_refit_adaptive_lines import request

HERE = Path(__file__).resolve().parent


class AdaptiveLinearRefitTests(unittest.TestCase):
    def test_request_archives_coverage_and_source_preservation(self):
        encoded = (HERE / 'fixtures/loft_refit_adaptive_lines.json').read_bytes()
        query = json.loads(encoded)
        native = json.loads((HERE / 'observations/loft_refit_adaptive_lines.json').read_text())
        self.assertEqual(query, request())
        self.assertEqual(len(query['operations']), 36)
        self.assertEqual([o['id'] for o in query['operations']],
                         [r['id'] for r in native['results']])
        self.assertEqual(hashlib.sha256(encoded).hexdigest(), native['provenance']['request_sha256'])
        for name, digest in native['provenance']['sources'].items():
            data = gzip.decompress((HERE / ('observations/source_snapshots/' + digest + '.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(data).hexdigest(), digest, name)
        for row in native['results']:
            self.assertEqual(row['value']['before'], row['value']['after'])

    def test_scale_and_absolute_tolerance_distinguish_the_joined_native_basis(self):
        native = json.loads((HERE / 'observations/loft_refit_adaptive_lines.json').read_text())
        rows = {r['id']: r['value']['surfaces'][0] for r in native['results']}
        small = rows['loft-refit-adaptive-bend-1-0-0.001-1.0']
        self.assertEqual(small['degree'][1], 1)
        ordinary = rows['loft-refit-adaptive-angle-0.25-1.0-1.0']
        large = rows['loft-refit-adaptive-angle-0.25-1000.0-1.0']
        self.assertEqual(ordinary['control_count'][1], 10)
        self.assertEqual(large['control_count'][1], 8)
        self.assertNotEqual(len(ordinary['knots_v']), len(large['knots_v']))


if __name__ == '__main__':
    unittest.main()
