"""Frozen Global Sweep1 captures and complete-distance classifications."""
from tools.verify_provenance import verify_measurement
from .qualification_sources import with_shared_sources
import gzip
import hashlib
import json
from pathlib import Path
import unittest
from .generate_sweep1_global_shapes import request as shapes_request
from .generate_sweep1_global_holdouts import request as holdouts_request

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent


class SweepGlobalShapesTests(unittest.TestCase):
    def test_native_captures_keep_exact_requests_and_frozen_producers(self):
        for family, build, count in [('sweep1_global_shapes', shapes_request, 11),
                                      ('sweep1_global_holdouts', holdouts_request, 7)]:
            fixture = HERE / ('fixtures/' + family + '.json')
            query = json.loads(fixture.read_text())
            data = json.loads((HERE / ('observations/' + family + '.json')).read_text())
            self.assertEqual(query, build())
            self.assertEqual(len(query['operations']), count)
            self.assertEqual([r['id'] for r in data['results']],
                             [op['id'] for op in query['operations']])
            self.assertEqual(hashlib.sha256(fixture.read_bytes()).hexdigest(),
                             data['provenance']['request_sha256'])
            for digest in data['provenance']['sources'].values():
                archive = HERE / ('observations/source_snapshots/' + digest + '.gz')
                self.assertEqual(hashlib.sha256(gzip.decompress(archive.read_bytes())).hexdigest(), digest)
            for row in data['results']:
                self.assertEqual(len(row['value']), 1)
                surface = row['value'][0]['definition']
                self.assertEqual(surface['degree'][1], 1)
                self.assertEqual(surface['control_count'][1], 2)
                self.assertTrue(all(c['weight'] == 1. for c in surface['control_points']))

    def test_historical_sources_cover_the_complete_distance_comparison(self):
        record = json.loads((ROOT / 'docs/sweep1/global-validation.json').read_text())
        self.assertGreater(verify_measurement(record, expand_sources=with_shared_sources), 0)
        for path in ['crates/viboceros-geometry/tests/sweep_global_shapes.rs',
                     'crates/viboceros-geometry/src/surface_distance.rs',
                     'crates/viboceros-geometry/src/surface_pullback/certificate/surface_minimum.rs']:
            self.assertIn(path, record['sha256'])

    def test_classifications_preserve_the_distinction_between_proofs_and_samples(self):
        record = json.loads((ROOT / 'docs/sweep1/global-validation.json').read_text())
        self.assertFalse(record['complete_rhino_sweep_compatibility'])
        self.assertFalse(record['global_multi_profile_policy_implemented'])
        self.assertEqual(len(record['cases']), 18)
        self.assertEqual(len({(r['family'], r['id']) for r in record['cases']}), 18)
        for row in record['cases']:
            self.assertEqual(row['comparison_epsilon'], 1e-6)
            self.assertEqual(row['construction_tolerance'], 1e-7)
            self.assertEqual(row['source_membership_error'], 1e-12)
            self.assertLessEqual(row['maximum_error'], 1e-9)
            self.assertIn(row['classification'], ['complete_images_within_epsilon',
                                                 'proved_gap', 'inconclusive'])
            if row['classification'] == 'proved_gap':
                self.assertGreater(row['image_distance_lower_bound'], row['comparison_epsilon'])
            if row['classification'] == 'complete_images_within_epsilon':
                self.assertLessEqual(row['complete_image_bound'], row['comparison_epsilon'])
        for name in ['rust', 'clippy']:
            self.assertEqual(record['checks'][name]['exit_code'], 0)


if __name__ == '__main__':
    unittest.main()
