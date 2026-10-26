"""Counterexamples separate local fits, parameter clocks and public constructors."""
import gzip
import hashlib
import json
from pathlib import Path
import unittest
from .generate_loft_refit_unequal_sampling import request as unequal
from .generate_loft_refit_interval_sampling import request as intervals
from .generate_loft_refit_interval_clocks import request as clocks
from .generate_loft_refit_sampling_stages import request as stages

HERE = Path(__file__).resolve().parent


def read(family):
    return json.loads((HERE / ('observations/' + family + '.json')).read_text())


class RefitSamplingStageTests(unittest.TestCase):
    def test_frozen_recipes_archives_and_source_preservation(self):
        for family, recipe, count in [
                ('loft_refit_unequal_sampling', unequal, 24),
                ('loft_refit_interval_sampling', intervals, 12),
                ('loft_refit_interval_clocks', clocks, 20),
                ('loft_refit_sampling_stages', stages, 6)]:
            encoded = (HERE / ('fixtures/' + family + '.json')).read_bytes()
            query, native = json.loads(encoded), read(family)
            self.assertEqual(query, recipe())
            self.assertEqual(len(query['operations']), count)
            self.assertEqual([o['id'] for o in query['operations']], [r['id'] for r in native['results']])
            self.assertEqual(hashlib.sha256(encoded).hexdigest(), native['provenance']['request_sha256'])
            for name, digest in native['provenance']['sources'].items():
                content = gzip.decompress((HERE / ('observations/source_snapshots/' + digest + '.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(content).hexdigest(), digest, name)
            for row in native['results']:
                self.assertEqual(row['value']['before'], row['value']['after'])

    def test_isolated_intervals_accept_one_cubic_despite_unequal_clocks_and_offsets(self):
        for family in ['loft_refit_interval_sampling', 'loft_refit_interval_clocks']:
            for row in read(family)['results']:
                self.assertEqual(row['value']['surfaces'][0]['control_count'][1], 4, row['id'])

    def test_public_bezier_constructor_is_not_the_joined_loft_subdivision(self):
        rows = read('loft_refit_sampling_stages')['results']
        self.assertEqual([len(r['value']['refit_beziers'][0]) for r in rows], [5, 3, 9, 5, 2, 2])
        self.assertEqual([r['value']['surfaces'][0]['control_count'][1] for r in rows], [12, 8, 16, 12, 4, 4])
        for row, counts in zip(rows, [(6, 12), (4, 8), (8, 16), (6, 12), (4, 4), (4, 4)]):
            source = row['value']['refit_sources']
            self.assertEqual(tuple(len(r['curves'][0]['control_points']) for r in source[:2]), counts)
            self.assertEqual(tuple(len(r['curves'][0]['control_points']) for r in source[2:4]), counts)


if __name__ == '__main__':
    unittest.main()
