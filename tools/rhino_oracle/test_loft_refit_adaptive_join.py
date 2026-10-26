"""Frozen native joining and inclusive minimum fitting tolerance evidence."""
import gzip
import hashlib
import json
from pathlib import Path
import unittest
from .generate_loft_refit_join_scales import request as scales
from .generate_loft_refit_join_curvature import request as curvature
from .generate_loft_refit_tiny_tolerances import request as tiny
from .generate_loft_refit_tolerance_floor import request as floor

HERE = Path(__file__).resolve().parent


def read(family):
    return json.loads((HERE / ('observations/' + family + '.json')).read_text())


class AdaptiveJoinTests(unittest.TestCase):
    def test_recipes_archives_order_and_native_input_preservation(self):
        for family, recipe, count in [
                ('loft_refit_join_scales', scales, 20),
                ('loft_refit_join_curvature', curvature, 11),
                ('loft_refit_tiny_tolerances', tiny, 24),
                ('loft_refit_tolerance_floor', floor, 18)]:
            encoded = (HERE / ('fixtures/' + family + '.json')).read_bytes()
            query, native = json.loads(encoded), read(family)
            self.assertEqual(query, recipe())
            self.assertEqual(len(query['operations']), count)
            self.assertEqual([o['id'] for o in query['operations']],
                             [r['id'] for r in native['results']])
            self.assertEqual(hashlib.sha256(encoded).hexdigest(), native['provenance']['request_sha256'])
            for name, digest in native['provenance']['sources'].items():
                content = gzip.decompress((HERE / ('observations/source_snapshots/' + digest + '.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(content).hexdigest(), digest, name)
            for row in native['results']:
                self.assertEqual(row['value']['before'], row['value']['after'])

    def test_native_fitting_floor_is_inclusive_and_preserves_all_three_source_types(self):
        query, native = tiny(), read('loft_refit_tiny_tolerances')
        disabled = {name: next(r['value']['surfaces'] for o, r in zip(query['operations'], native['results'])
                              if o['id'] == 'loft-refit-tiny-tolerance-' + name + '-0.0')
                    for name in ['bend', 'quarter', 'line']}
        for recipe, family in [(tiny, 'loft_refit_tiny_tolerances'), (floor, 'loft_refit_tolerance_floor')]:
            for op, row in zip(recipe()['operations'], read(family)['results']):
                name = next(name for name in disabled if '-' + name + '-' in op['id'])
                self.assertEqual(row['value']['surfaces'] == disabled[name],
                                 op['refit_tolerance'] <= 2. ** -25, op['id'])

    def test_absolute_curvature_tolerance_changes_native_join_multiplicity_with_scale(self):
        rows = read('loft_refit_join_curvature')['results']
        counts = [r['value']['surfaces'][0]['control_count'][1] for r in rows]
        self.assertEqual(counts, [10, 10, 9, 9, 9, 9, 8, 8, 10, 9, 8])


if __name__ == '__main__':
    unittest.main()
