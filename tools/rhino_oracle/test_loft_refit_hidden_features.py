"""Frozen evidence that paired fitting examines more than nine length stations."""
import bisect
import gzip
import hashlib
import json
from pathlib import Path
import unittest
from .generate_loft_refit_hidden_features import request
from .test_source_continuity import difference

HERE = Path(__file__).resolve().parent


def point_at(curve, parameter):
    positions = curve['knots'][1:-1]
    i = max(0, min(bisect.bisect_right(positions, parameter) - 1, len(positions) - 2))
    f = (parameter - positions[i]) / (positions[i + 1] - positions[i])
    a, b = [c['point'] for c in curve['control_points'][i:i + 2]]
    return [x + f * (y - x) for x, y in zip(a, b)]


class HiddenFeatureTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.encoded = (HERE / 'fixtures/loft_refit_hidden_features.json').read_bytes()
        cls.query = json.loads(cls.encoded)
        cls.native = json.loads((HERE / 'observations/loft_refit_hidden_features.json').read_text())

    def test_recipe_archives_order_and_source_purity(self):
        self.assertEqual(self.query, request())
        self.assertEqual(len(self.query['operations']), 16)
        self.assertEqual([o['id'] for o in self.query['operations']],
                         [r['id'] for r in self.native['results']])
        self.assertEqual(hashlib.sha256(self.encoded).hexdigest(), self.native['provenance']['request_sha256'])
        for name, digest in self.native['provenance']['sources'].items():
            data = gzip.decompress((HERE / ('observations/source_snapshots/' + digest + '.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(data).hexdigest(), digest, name)
        for row in self.native['results']:
            self.assertEqual(row['value']['before'], row['value']['after'])

    def test_native_output_retains_features_missed_by_all_nine_length_stations(self):
        for op, row in zip(self.query['operations'], self.native['results']):
            source = op['curves'][0]
            record = row['value']['refit_sources'][1]
            parameters = record['numerical_clocks'][0]['batch_parameters']
            self.assertEqual(len(parameters), 9)
            self.assertEqual([point_at(source, t)[1] for t in parameters], [0.] * 9)
            fitted = record['curves'][0]
            peak = max(abs(p['point'][1]) for p in source['control_points'])
            self.assertGreater(max(abs(p['point'][1]) for p in fitted['control_points']), peak * .5)
            self.assertGreater(len(fitted['control_points']), 4)
            self.assertEqual(row['value']['surfaces'][0]['control_count'][1], len(fitted['control_points']))

    def test_source_clocks_change_paired_topology_while_simplification_preserves_it(self):
        operations, rows = self.query['operations'], self.native['results']
        for i in range(0, 16, 2):
            a, b = [r['value']['refit_sources'] for r in rows[i:i + 2]]
            self.assertNotEqual(len(a[1]['curves'][0]['control_points']), len(b[1]['curves'][0]['control_points']))
            for raw in [a, b]:
                for method in [0, 1]:
                    self.assertLessEqual(difference(raw[method]['curves'], raw[method + 2]['curves']), 1e-9)
            pa = a[1]['numerical_clocks'][0]['batch_parameters']
            pb = b[1]['numerical_clocks'][0]['batch_parameters']
            for ta, tb in zip(pa, pb):
                x = point_at(operations[i]['curves'][0], ta)
                y = point_at(operations[i + 1]['curves'][0], tb)
                self.assertLessEqual(max(abs(v - w) for v, w in zip(x, y)), 1e-9)


if __name__ == '__main__':
    unittest.main()
