"""Immutable fitting-clock evidence and explicit scalar limitations."""
import gzip
import hashlib
import json
from pathlib import Path
import unittest
from .generate_loft_refit_frames import request as frames
from .generate_loft_scalar_clock import request as scalar
from .generate_loft_fitting_clock import request as fitting

HERE = Path(__file__).resolve().parent


class FittingClockTests(unittest.TestCase):
    def test_recipes_source_preservation_and_producer_archives(self):
        for family, recipe, count in [('loft_refit_frames', frames, 6),
                ('loft_scalar_clock', scalar, 19), ('loft_fitting_clock', fitting, 19)]:
            path = HERE / ('fixtures/' + family + '.json')
            query = json.loads(path.read_text())
            data = json.loads((HERE / ('observations/' + family + '.json')).read_text())
            self.assertEqual(query, recipe())
            self.assertEqual(len(query['operations']), count)
            self.assertEqual([o['id'] for o in query['operations']], [r['id'] for r in data['results']])
            self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(), data['provenance']['request_sha256'])
            for name, digest in data['provenance']['sources'].items():
                content = gzip.decompress((HERE / ('observations/source_snapshots/' + digest + '.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(content).hexdigest(), digest, name)
            for row in data['results']:
                self.assertEqual(row['value']['before'], row['value']['after'])

    def test_scalar_reference_is_independent_of_query_order(self):
        data = json.loads((HERE / 'observations/loft_scalar_clock.json').read_text())
        for row in data['results']:
            for clock in row['value']['scalar_clock']:
                self.assertEqual(clock['forward'], list(reversed(clock['reverse'])))
                self.assertTrue(all(s['success'] for s in clock['forward']))

    def test_two_coincident_inputs_reproduce_translated_pair_fitting(self):
        data = json.loads((HERE / 'observations/loft_refit_frames.json').read_text())
        for row in data['results']:
            variants = {v['variant']: v['methods'] for v in row['value']['refit_frames']}
            self.assertEqual(variants['coincident'][1]['curves'][0], variants['translated'][1]['curves'][0])
            self.assertEqual(variants['single'][0]['curves'][0], variants['coincident'][0]['curves'][0])
            self.assertNotEqual(variants['single'][1]['curves'][0]['control_points'],
                                variants['coincident'][1]['curves'][0]['control_points'])


if __name__ == '__main__':
    unittest.main()
