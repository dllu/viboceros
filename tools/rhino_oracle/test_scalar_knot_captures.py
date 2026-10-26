"""Frozen numerical knot acceptance and distinct fitting midpoint references."""
import gzip
import hashlib
import json
from pathlib import Path
import unittest
from .generate_loft_refit_midpoint_snapping import request as snapping
from .generate_loft_refit_midpoint_clocks import request as clocks
from .generate_loft_refit_midpoint_boundary import request as boundary
from .generate_scalar_knot_holdouts import request as holdouts

HERE = Path(__file__).resolve().parent


def read(family):
    return json.loads((HERE / ('observations/' + family + '.json')).read_text())


class ScalarKnotCaptureTests(unittest.TestCase):
    def test_recipes_archives_and_native_inputs_are_immutable(self):
        for family, recipe, count in [
                ('loft_refit_midpoint_snapping', snapping, 30),
                ('loft_refit_midpoint_clocks', clocks, 36),
                ('loft_refit_midpoint_boundary', boundary, 32),
                ('scalar_knot_holdouts', holdouts, 36)]:
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

    def test_scalar_and_incremental_batch_have_different_near_knot_acceptance(self):
        row = next(r for r in read('loft_refit_midpoint_clocks')['results']
                   if r['id'] == 'loft-refit-midpoint-clock-1e-05-0-0.0001')['value']
        scalar = next(c for c in row['scalar_clock'] if c['fractional_tolerance'] == 1e-4)
        midpoint = next(s['parameter'] for s in scalar['forward'] if s['fraction'] == .5)
        self.assertEqual(midpoint, 1.)
        self.assertGreater(abs(row['refit_adjustment']['parameters'][4] - midpoint), 1e-6)

    def test_loose_scalar_checkpoint_does_not_determine_paired_fitting_subdivision(self):
        row = next(r for r in read('loft_refit_midpoint_boundary')['results']
                   if r['id'] == 'loft-refit-midpoint-boundary--0.02-0')['value']
        scalar = row['scalar_clock'][-1]
        self.assertEqual(scalar['fractional_tolerance'], .01)
        self.assertEqual(next(s['parameter'] for s in scalar['forward'] if s['fraction'] == .5), 1.)
        knots = sorted(set(row['surfaces'][0]['knots_v']))
        self.assertEqual([round(k / knots[-1], 10) for k in knots], [0., .5, .625, .75, 1.])

    def test_nearby_plateau_queries_use_the_right_parameter(self):
        rows = {r['id']: r['value'] for r in read('scalar_knot_holdouts')['results']}
        row = rows['scalar-knot-holdout-plateau-0.0001']
        self.assertEqual(row['parameters'][4:7], [2., 2., 2.])
        self.assertEqual(row['parameters'][0], -3.)
        self.assertEqual(row['parameters'][-1], 7.)


if __name__ == '__main__':
    unittest.main()
