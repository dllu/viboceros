"""Preserve source-type clock distinctions without inferring a fitter rule."""
import gzip
import hashlib
import json
from pathlib import Path
import unittest
from .generate_loft_refit_component_clocks import request

HERE = Path(__file__).resolve().parent


def read(folder, family):
    return json.loads((HERE / (folder + '/' + family + '.json')).read_text())


class ComponentClockTests(unittest.TestCase):
    def test_frozen_sources_request_and_native_inputs(self):
        family = 'loft_refit_component_clocks'
        encoded = (HERE / ('fixtures/' + family + '.json')).read_bytes()
        query, native = json.loads(encoded), read('observations', family)
        self.assertEqual(query, request())
        self.assertEqual(len(query['operations']), 6)
        self.assertEqual([o['id'] for o in query['operations']], [r['id'] for r in native['results']])
        self.assertEqual(hashlib.sha256(encoded).hexdigest(), native['provenance']['request_sha256'])
        for name, digest in native['provenance']['sources'].items():
            content = gzip.decompress((HERE / ('observations/source_snapshots/' + digest + '.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(content).hexdigest(), digest, name)
        for row in native['results']:
            self.assertEqual(row['value']['before'], row['value']['after'])

    def test_polyline_and_nurbs_scalar_policies_differ_on_equivalent_source_images(self):
        rows = read('observations', 'loft_refit_component_clocks')['results']
        row = rows[0]['value']['refit_sources']
        original = row[0]['numerical_clocks'][-1]['scalars'][1]['parameter']
        simplified = row[2]['numerical_clocks'][-1]['scalars'][1]['parameter']
        self.assertEqual(row[0]['source_types'][0], 'NurbsCurve')
        self.assertEqual(row[2]['source_types'][0], 'PolylineCurve')
        self.assertEqual(original, 1.)
        self.assertLess(simplified, .991)
        for record in rows:
            sources = record['value']['refit_sources']
            self.assertEqual(sources[0]['numerical_clocks'], sources[1]['numerical_clocks'])
            self.assertEqual(sources[2]['numerical_clocks'], sources[3]['numerical_clocks'])
            self.assertEqual(sources[0]['curves'], sources[2]['curves'])
            self.assertEqual(sources[1]['curves'], sources[3]['curves'])


if __name__ == '__main__':
    unittest.main()
