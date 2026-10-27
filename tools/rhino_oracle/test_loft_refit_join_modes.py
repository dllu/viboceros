"""Frozen distinction between global reconstruction and independent regions."""
import gzip
import hashlib
import itertools
import json
from pathlib import Path
import unittest
from .generate_loft_refit_join_modes import request

HERE = Path(__file__).resolve().parent
FAMILY = 'loft_refit_join_modes'


class JoinModeTests(unittest.TestCase):
    def test_recipe_archives_and_original_definitions(self):
        encoded = (HERE / ('fixtures/' + FAMILY + '.json')).read_bytes()
        query = json.loads(encoded)
        native = json.loads((HERE / ('observations/' + FAMILY + '.json')).read_text())
        self.assertEqual(query, request())
        self.assertEqual(len(query['operations']), 16)
        self.assertEqual([op['id'] for op in query['operations']],
                         [row['id'] for row in native['results']])
        self.assertEqual(hashlib.sha256(encoded).hexdigest(),
                         native['provenance']['request_sha256'])
        for name, digest in native['provenance']['sources'].items():
            path = HERE / ('observations/source_snapshots/' + digest + '.gz')
            self.assertEqual(hashlib.sha256(gzip.decompress(path.read_bytes())).hexdigest(),
                             digest, name)
        for row in native['results']:
            self.assertEqual(row['value']['before'], row['value']['after'])

    def test_only_single_region_fits_retain_the_original_dyadic_chart(self):
        native = json.loads((HERE / ('observations/' + FAMILY + '.json')).read_text())
        single = multiple = 0
        counts = []
        for row in native['results']:
            curve = row['value']['refit_sources'][1]['curves'][0]
            counts.append(len(curve['control_points']))
            multiplicities = [len(list(group)) for _, group in itertools.groupby(curve['knots'])]
            if 3 in multiplicities:
                multiple += 1
                self.assertNotEqual(curve['domain'], [0., 1.])
            else:
                single += 1
                self.assertEqual(curve['domain'], [0., 1.])
                self.assertTrue(all(t * 128 == round(t * 128) for t in curve['knots']))
        self.assertEqual((single, multiple), (10, 6))
        self.assertEqual(counts, [35, 35, 28, 28, 44, 39, 34, 34,
                                  38, 34, 34, 34, 21, 20, 20, 20])


if __name__ == '__main__':
    unittest.main()
