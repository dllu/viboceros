"""Frozen public SDK angle overloads, with disabled-fit chart holdouts."""
import gzip
import hashlib
import json
from pathlib import Path
import unittest
from .generate_loft_refit_angles import request as angles
from .generate_loft_refit_angle_holdouts import request as holdouts

HERE = Path(__file__).resolve().parent


class LoftRefitAngleTests(unittest.TestCase):
    def test_recipes_coverage_frozen_producers_and_source_preservation(self):
        for family, recipe, count in [('loft_refit_angles', angles, 176),
                                     ('loft_refit_angle_holdouts', holdouts, 41)]:
            encoded = (HERE / ('fixtures/' + family + '.json')).read_bytes()
            query = json.loads(encoded)
            native = json.loads((HERE / ('observations/' + family + '.json')).read_text())
            self.assertEqual(query, recipe())
            self.assertEqual(len(query['operations']), count)
            self.assertEqual([o['id'] for o in query['operations']],
                             [r['id'] for r in native['results']])
            self.assertEqual(hashlib.sha256(encoded).hexdigest(),
                             native['provenance']['request_sha256'])
            for name, digest in native['provenance']['sources'].items():
                content = gzip.decompress((HERE / ('observations/source_snapshots/'
                                                   + digest + '.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(content).hexdigest(), digest, name)
            for row in native['results']:
                self.assertEqual(row['value']['before'], row['value']['after'])

    def test_disabled_fit_still_changes_higher_degree_section_parameters(self):
        native = json.loads((HERE / 'observations/loft_refit_angle_holdouts.json').read_text())
        rows = {r['id']: r['value']['surfaces'][0] for r in native['results']
                if '-chart-' in r['id']}
        for degree in [2, 3]:
            for domains in [0, 1]:
                prefix = 'loft-refit-angle-chart-{}-{}-'.format(degree, domains)
                default = rows[prefix + '0']
                for index in [1, 2, 4]:
                    self.assertEqual(default, rows[prefix + str(index)])
                suppressed = rows[prefix + '3']
                self.assertEqual(default['control_points'], suppressed['control_points'])
                self.assertNotEqual(default['knots_v'], suppressed['knots_v'])
                self.assertNotEqual(default['samples'], suppressed['samples'])

    def test_explicit_acute_angles_keep_single_bezier_smooth_fit_records(self):
        native = json.loads((HERE / 'observations/loft_refit_angle_holdouts.json').read_text())
        rows = {r['id']: r['value']['surfaces'] for r in native['results']}
        for shape in ['quarter', 'quadratic_2', 'polynomial_cubic']:
            prefix = 'loft-refit-angle-smooth-' + shape + '-'
            for index in range(1, 5):
                self.assertEqual(rows[prefix + '0'], rows[prefix + str(index)])
            for index in [5, 6]:
                self.assertEqual(rows[prefix + str(index)], [])


if __name__ == '__main__':
    unittest.main()
