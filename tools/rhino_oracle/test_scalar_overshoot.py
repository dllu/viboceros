"""Immutable public-SDK evidence for scalar inverse checkpoint selection."""
import gzip
import hashlib
import json
from pathlib import Path
import unittest
from .generate_scalar_overshoot import request

HERE = Path(__file__).resolve().parent


class ScalarOvershootTests(unittest.TestCase):
    def test_recipe_archives_and_native_source_purity(self):
        encoded = (HERE / 'fixtures/scalar_overshoot.json').read_bytes()
        query = json.loads(encoded)
        native = json.loads((HERE / 'observations/scalar_overshoot.json').read_text())
        self.assertEqual(query, request())
        self.assertEqual(len(query['operations']), 24)
        self.assertEqual([o['id'] for o in query['operations']], [r['id'] for r in native['results']])
        self.assertEqual(hashlib.sha256(encoded).hexdigest(),native['provenance']['request_sha256'])
        for name, digest in native['provenance']['sources'].items():
            source = gzip.decompress((HERE / ('observations/source_snapshots/'+digest+'.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(source).hexdigest(), digest, name)
        for row in native['results']:
            self.assertEqual(row['value']['before'],row['value']['after'])

    def test_query_order_and_affine_domains_preserve_the_captured_counterexample(self):
        native = json.loads((HERE / 'observations/scalar_overshoot.json').read_text())
        rows = {r['id']:r['value'] for r in native['results']}
        for value in rows.values():
            self.assertEqual(value['parameters'][:16],list(reversed(value['parameters'][16:])))
            self.assertEqual(value['points'][:16],list(reversed(value['points'][16:])))
        base = rows['scalar-overshoot-cubic-base-0.01']
        shifted = rows['scalar-overshoot-cubic-shifted-0.01']
        self.assertAlmostEqual(base['parameters'][5],0.002586795855915225,delta=1e-12)
        self.assertGreater(abs(base['parameters'][5]-0.008344502761016854),.005)
        self.assertAlmostEqual(shifted['parameters'][5],-3.+10.*base['parameters'][5],delta=1e-12)


if __name__=='__main__':
    unittest.main()
