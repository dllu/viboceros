"""Retain corner-angle boundaries without conflating them with fit distance."""
import gzip
import hashlib
import json
from pathlib import Path
import unittest
from .generate_loft_refit_shallow_angles import request as angles
from .generate_loft_refit_shallow_holdouts import request as holdouts
from .generate_loft_refit_corner_boundary import request as boundary
from .generate_loft_refit_corner_suppression import request as suppression
from .generate_loft_refit_suppressed_initial import request as initial
from .generate_loft_refit_shallow_styles import request as styles

HERE = Path(__file__).resolve().parent


def read(family):
    return json.loads((HERE / ('observations/' + family + '.json')).read_text())


class ShallowRefitAngleTests(unittest.TestCase):
    def test_recipes_frozen_sources_and_native_inputs(self):
        for family, recipe, count in [
                ('loft_refit_shallow_angles', angles, 36),
                ('loft_refit_shallow_holdouts', holdouts, 48),
                ('loft_refit_corner_boundary', boundary, 22),
                ('loft_refit_corner_suppression', suppression, 25),
                ('loft_refit_suppressed_initial', initial, 12),
                ('loft_refit_shallow_styles', styles, 14)]:
            encoded = (HERE / ('fixtures/' + family + '.json')).read_bytes()
            query = json.loads(encoded)
            native = read(family)
            self.assertEqual(query, recipe())
            self.assertEqual(len(query['operations']), count)
            self.assertEqual([o['id'] for o in query['operations']],
                             [r['id'] for r in native['results']])
            self.assertEqual(hashlib.sha256(encoded).hexdigest(), native['provenance']['request_sha256'])
            for name, digest in native['provenance']['sources'].items():
                content = gzip.decompress((HERE / ('observations/source_snapshots/'
                                                   + digest + '.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(content).hexdigest(), digest, name)
            for row in native['results']:
                self.assertEqual(row['value']['before'], row['value']['after'])

    def test_zero_uses_default_but_small_positive_angles_retain_shallow_corners(self):
        rows = {r['id']: r['value']['surfaces'][0] for r in read('loft_refit_shallow_angles')['results']}
        for bend in [.25, .9]:
            prefix = 'loft-refit-shallow-angle-{}-0.0'.format(bend)
            self.assertEqual(rows[prefix]['control_count'][1], 7)
            self.assertGreater(rows[prefix + '-zero-angle']['control_count'][1], 7)
        for bend in [.03, .05, .08, .1]:
            for factor in [0., .5, 1.5]:
                key = 'loft-refit-shallow-angle-{}-{}'.format(bend, factor)
                self.assertGreater(rows[key]['control_count'][1], 7)

    def test_the_intrinsic_boundary_has_a_scale_sensitive_exact_edge(self):
        rows = {r['id']: r['value']['surfaces'][0] for r in read('loft_refit_corner_boundary')['results']}
        for scale in [1., 1000.]:
            for bend in [.0018, .0019, .00199]:
                self.assertGreater(rows['loft-refit-corner-boundary-{}-{}'.format(bend, scale)]['control_count'][1], 7)
            for bend in [.00200001, .00201, .0021, .0022, .0023, .0024, .0025]:
                self.assertEqual(rows['loft-refit-corner-boundary-{}-{}'.format(bend, scale)]['control_count'][1], 7)
        self.assertGreater(rows['loft-refit-corner-boundary-0.002-1.0']['control_count'][1], 7)
        self.assertEqual(rows['loft-refit-corner-boundary-0.002-1000.0']['control_count'][1], 7)

    def test_requested_angles_above_the_intrinsic_floor_still_suppress_corners(self):
        query = suppression()
        native = read('loft_refit_corner_suppression')
        retained = 0
        for op, row in zip(query['operations'], native['results']):
            bend = float(op['id'].split('-')[-2])
            count = row['value']['surfaces'][0]['control_count'][1]
            resolved = bend > max(.002, op['refit_angle_tolerance'])
            self.assertEqual(count == 7, resolved)
            retained += resolved
        self.assertEqual(retained, 15)


if __name__ == '__main__':
    unittest.main()
