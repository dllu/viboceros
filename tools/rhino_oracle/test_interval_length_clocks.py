"""Frozen interval clocks and source-parameter-sensitive adaptive references."""
import gzip
import hashlib
import json
from pathlib import Path
import unittest
from .generate_interval_length_clocks import request as intervals
from .generate_loft_refit_adaptive_parameter_clocks import request as adaptive

HERE = Path(__file__).resolve().parent


def read(folder, family):
    return json.loads((HERE / (folder + '/' + family + '.json')).read_text())


class IntervalClockTests(unittest.TestCase):
    def test_frozen_recipes_archives_order_and_unchanged_native_sources(self):
        for family, recipe, count in [('interval_length_clocks', intervals, 96),
                ('loft_refit_adaptive_parameter_clocks', adaptive, 24)]:
            encoded = (HERE / ('fixtures/' + family + '.json')).read_bytes()
            query, native = json.loads(encoded), read('observations', family)
            self.assertEqual(query, recipe())
            self.assertEqual(len(query['operations']), count)
            self.assertEqual([o['id'] for o in query['operations']], [r['id'] for r in native['results']])
            self.assertEqual(hashlib.sha256(encoded).hexdigest(), native['provenance']['request_sha256'])
            for name, digest in native['provenance']['sources'].items():
                content = gzip.decompress((HERE / ('observations/source_snapshots/' + digest + '.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(content).hexdigest(), digest, name)
            for row in native['results']:
                self.assertEqual(row['value']['before'], row['value']['after'])

    def test_native_interval_parameters_retain_source_chart_and_query_order_independence(self):
        query = intervals()
        for op, row in zip(query['operations'], read('observations', 'interval_length_clocks')['results']):
            value = row['value']
            self.assertEqual(value['domain'], op['subdomain'])
            self.assertEqual(value['parameters'][0], op['subdomain'][0])
            self.assertEqual(value['parameters'][8], op['subdomain'][1])
            self.assertEqual(value['parameters'][4], value['parameters'][9])
            self.assertEqual(value['parameters'][2], value['parameters'][10])
            self.assertTrue(all(op['subdomain'][0] <= t <= op['subdomain'][1] for t in value['parameters']))

    def test_equal_source_images_can_have_different_native_fitted_topologies(self):
        query = adaptive()['operations']
        rows = read('observations', 'loft_refit_adaptive_parameter_clocks')['results']
        for start, counts in [(0, [16, 12, 14, 14]), (4, [12, 10, 12, 10]), (8, [8, 8, 8, 8]),
                (12, [14, 12, 14, 16]), (16, [10, 12, 12, 12]), (20, [8, 8, 8, 8])]:
            source = query[start]['curves'][0]['control_points']
            self.assertTrue(all(op['curves'][0]['control_points'] == source for op in query[start:start + 4]))
            actual = [r['value']['surfaces'][0]['control_count'][1] for r in rows[start:start + 4]]
            self.assertEqual(actual, counts)


if __name__ == '__main__':
    unittest.main()
