"""Immutable public profile-set references for continuous curve comparisons."""
import gzip
import hashlib
import json
from pathlib import Path
import unittest
from .generate_loft_refit_sets import request

HERE = Path(__file__).resolve().parent


class RefitSetTests(unittest.TestCase):
    def test_recipe_archives_and_unmodified_sources(self):
        path = HERE / 'fixtures/loft_refit_sets.json'
        query = json.loads(path.read_text())
        data = json.loads((HERE / 'observations/loft_refit_sets.json').read_text())
        self.assertEqual(query, request())
        self.assertEqual(len(query['operations']), 6)
        self.assertEqual([o['id'] for o in query['operations']], [r['id'] for r in data['results']])
        self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(), data['provenance']['request_sha256'])
        for name, digest in data['provenance']['sources'].items():
            archive = HERE / ('observations/source_snapshots/' + digest + '.gz')
            self.assertEqual(hashlib.sha256(gzip.decompress(archive.read_bytes())).hexdigest(), digest, name)
        for row in data['results']:
            self.assertEqual(row['value']['before'], row['value']['after'])

    def test_equal_translated_profile_count_does_not_change_the_first_prepared_curve(self):
        data = json.loads((HERE / 'observations/loft_refit_sets.json').read_text())
        for row in data['results']:
            variants = {v['variant']: v for v in row['value']['refit_sets']}
            self.assertEqual(len(variants), 9)
            for variant in variants.values():
                self.assertEqual(len(variant['curves']), variant['count'])
            for name in ['triple', 'quadruple']:
                self.assertEqual(variants['paired']['curves'][0], variants[name]['curves'][0])
            # This is an observed policy, not an inference from control counts:
            # the complete stored definition of profile zero agrees exactly.
            self.assertNotEqual(variants['single']['curves'][0], variants['paired']['curves'][0])


if __name__ == '__main__':
    unittest.main()
