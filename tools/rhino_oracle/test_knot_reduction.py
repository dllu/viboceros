"""Frozen SDK evidence, including the degree-one tolerance counterexample."""
import gzip
import hashlib
import json
from pathlib import Path
import unittest
from .generate_knot_reduction import request

HERE = Path(__file__).resolve().parent


class KnotReductionTests(unittest.TestCase):
    def test_recipe_archives_and_source_purity(self):
        encoded = (HERE / 'fixtures/knot_reduction.json').read_bytes()
        query = json.loads(encoded)
        native = json.loads((HERE / 'observations/knot_reduction.json').read_text())
        self.assertEqual(query, request())
        self.assertEqual(len(query['operations']), 140)
        self.assertEqual([o['id'] for o in query['operations']], [r['id'] for r in native['results']])
        self.assertEqual(hashlib.sha256(encoded).hexdigest(), native['provenance']['request_sha256'])
        for name, digest in native['provenance']['sources'].items():
            source = gzip.decompress((HERE / ('observations/source_snapshots/' + digest + '.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(source).hexdigest(), digest, name)
        for row in native['results']:
            self.assertEqual(row['value']['before'], row['value']['after'])

    def test_zero_is_unrestricted_and_minimum_selects_whole_groups(self):
        data = json.loads((HERE / 'observations/knot_reduction.json').read_text())
        rows = {r['id']: r['value'] for r in data['results']}
        prefix = 'knot-reduction-fresh-half-join-2-4-'
        self.assertEqual(rows[prefix + '0.0']['removed'], 2)
        self.assertEqual(rows[prefix + '1e-09']['removed'], 0)
        self.assertEqual(rows[prefix + '10.0']['removed'], 2)
        self.assertEqual(len(rows[prefix + '0.0']['curve']['control_points']), 5)

    def test_sdk_positive_tolerance_does_not_protect_polyline_corners(self):
        data = json.loads((HERE / 'observations/knot_reduction.json').read_text())
        witnessed = 0
        for row in data['results']:
            if not row['id'].startswith('knot-reduction-linear-') or not row['id'].endswith('-1e-09'):
                continue
            value = row['value']
            if not value['removed']:
                continue
            self.assertEqual(value['removed'], 3)
            controls = value['curve']['control_points']
            self.assertEqual(len(controls), 2)
            a, b = [c['point'] for c in controls]
            point = value['before']['control_points'][1]['point']
            edge = [y - x for x, y in zip(a, b)]
            delta = [x - y for x, y in zip(point, a)]
            t = max(0., min(1., sum(x*y for x, y in zip(delta, edge)) / sum(x*x for x in edge)))
            distance = sum((d-t*e)**2 for d, e in zip(delta, edge))**.5
            self.assertGreater(distance, 3.)
            witnessed += 1
        self.assertEqual(witnessed, 2)


if __name__ == '__main__':
    unittest.main()
