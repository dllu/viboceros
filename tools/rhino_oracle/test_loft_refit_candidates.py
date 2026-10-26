"""Immutable native evidence for initial smooth Refit proposals."""
import gzip
import hashlib
import json
from pathlib import Path
import unittest
from .generate_loft_refit_coarse import request as coarse
from .generate_loft_refit_candidates import request as candidates
from .generate_loft_refit_candidate_boundary import request as boundary

HERE = Path(__file__).resolve().parent


class InitialSmoothRefitTests(unittest.TestCase):
    def test_recipes_archives_and_source_definitions(self):
        for family, recipe, count in [('loft_refit_coarse', coarse, 39),
                ('loft_refit_candidates', candidates, 80),
                ('loft_refit_candidate_boundary', boundary, 16)]:
            encoded = (HERE / ('fixtures/' + family + '.json')).read_bytes()
            query = json.loads(encoded)
            native = json.loads((HERE / ('observations/' + family + '.json')).read_text())
            self.assertEqual(query, recipe())
            self.assertEqual(len(query['operations']), count)
            self.assertEqual([o['id'] for o in query['operations']], [r['id'] for r in native['results']])
            self.assertEqual(hashlib.sha256(encoded).hexdigest(), native['provenance']['request_sha256'])
            for name, digest in native['provenance']['sources'].items():
                source = gzip.decompress((HERE / ('observations/source_snapshots/' + digest + '.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(source).hexdigest(), digest, name)
            for row in native['results']:
                self.assertEqual(row['value']['before'], row['value']['after'])

    def test_native_transition_straddles_the_reserved_half_tolerance(self):
        native = json.loads((HERE / 'observations/loft_refit_candidate_boundary.json').read_text())
        rows = native['results']
        self.assertEqual(len(rows), 16)
        for below, above in zip(rows[::2], rows[1::2]):
            self.assertEqual(below['id'].removesuffix('-below'), above['id'].removesuffix('-above'))
            a = below['value']['surfaces'][0]
            b = above['value']['surfaces'][0]
            self.assertEqual(a['control_count'], b['control_count'])
            self.assertEqual(a['control_count'][1], 4)
            nu = a['control_count'][0]
            p, q = a['control_points'][nu]['point'], b['control_points'][nu]['point']
            self.assertGreater(max(abs(x-y) for x, y in zip(p, q)), .01)


if __name__ == '__main__':
    unittest.main()
