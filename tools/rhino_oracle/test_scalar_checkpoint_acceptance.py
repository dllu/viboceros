"""Frozen endpoint scalar inverses, including query order and source purity."""
import gzip
import hashlib
import json
from pathlib import Path
import unittest
from .generate_scalar_checkpoint_acceptance import request
from .generate_scalar_endpoint_boundaries import request as boundaries

HERE = Path(__file__).resolve().parent


class ScalarCheckpointAcceptanceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.encoded = (HERE / 'fixtures/scalar_checkpoint_acceptance.json').read_bytes()
        cls.query = json.loads(cls.encoded)
        cls.native = json.loads((HERE / 'observations/scalar_checkpoint_acceptance.json').read_text())

    def test_recipe_archives_and_source_purity(self):
        self.assertEqual(self.query, request())
        self.assertEqual(len(self.query['operations']), 63)
        self.assertEqual([o['id'] for o in self.query['operations']],
                         [r['id'] for r in self.native['results']])
        self.assertEqual(hashlib.sha256(self.encoded).hexdigest(),
                         self.native['provenance']['request_sha256'])
        for name, digest in self.native['provenance']['sources'].items():
            content = gzip.decompress((HERE / ('observations/source_snapshots/' + digest + '.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(content).hexdigest(), digest, name)
        for row in self.native['results']:
            self.assertEqual(row['value']['before'], row['value']['after'])

    def test_native_queries_are_independent_of_order_and_repetition(self):
        count = 0
        for op, row in zip(self.query['operations'], self.native['results']):
            value, seen = row['value'], {}
            self.assertEqual(len(value['parameters']), len(op['fractions']))
            for f, t, p in zip(op['fractions'], value['parameters'], value['points']):
                if f in seen:
                    self.assertEqual(seen[f], (t, p))
                seen[f] = (t, p)
                self.assertLessEqual(value['domain'][0], t)
                self.assertLessEqual(t, value['domain'][1])
                if f in [0., 1.]:
                    self.assertEqual(t, value['domain'][int(f)])
                count += 1
        self.assertEqual(count, 10647)

    def test_affine_source_domains_retain_the_same_physical_results(self):
        operations, rows = self.query['operations'], self.native['results']
        for first in range(0, 63, 9):
            for accuracy in range(3):
                a, b = rows[first + accuracy]['value'], rows[first + 3 + accuracy]['value']
                self.assertEqual(operations[first + accuracy]['fractional_tolerance'],
                                 operations[first + 3 + accuracy]['fractional_tolerance'])
                for t, shifted, p, q in zip(a['parameters'], b['parameters'], a['points'], b['points']):
                    self.assertLessEqual(abs(-3. + 10. * t - shifted), 1e-9)
                    self.assertLessEqual(max(abs(x - y) for x, y in zip(p, q)), 1e-9)

    def test_frozen_tolerance_boundaries_and_overlapping_endpoint_ranges(self):
        encoded = (HERE / 'fixtures/scalar_endpoint_boundaries.json').read_bytes()
        query = json.loads(encoded)
        native = json.loads((HERE / 'observations/scalar_endpoint_boundaries.json').read_text())
        self.assertEqual(query, boundaries())
        self.assertEqual(len(query['operations']), 66)
        self.assertEqual([o['id'] for o in query['operations']], [r['id'] for r in native['results']])
        self.assertEqual(hashlib.sha256(encoded).hexdigest(), native['provenance']['request_sha256'])
        for name, digest in native['provenance']['sources'].items():
            content = gzip.decompress((HERE / ('observations/source_snapshots/' + digest + '.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(content).hexdigest(), digest, name)
        count = 0
        for op, row in zip(query['operations'], native['results']):
            value, accuracy = row['value'], op['fractional_tolerance']
            self.assertEqual(value['before'], value['after'])
            for f, t in zip(op['fractions'], value['parameters']):
                if f <= accuracy:
                    self.assertEqual(t, value['domain'][0])
                elif 1. - f <= accuracy:
                    self.assertEqual(t, value['domain'][1])
                count += 1
        self.assertEqual(count, 1180)


if __name__ == '__main__':
    unittest.main()
