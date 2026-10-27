"""Immutable public queries isolate numerical stopping from fitting choices."""
import gzip
import hashlib
import json
from pathlib import Path
import unittest
from .generate_loft_refit_closest_stations import request

HERE = Path(__file__).resolve().parent


class ClosestStationTests(unittest.TestCase):
    def test_recipe_archives_and_complete_query_order(self):
        encoded = (HERE / 'fixtures/loft_refit_closest_stations.json').read_bytes()
        query = json.loads(encoded)
        native = json.loads((HERE / 'observations/loft_refit_closest_stations.json').read_text())
        self.assertEqual(query, request())
        self.assertEqual(len(query['operations']), 147)
        self.assertEqual([o['id'] for o in query['operations']],
                         [r['id'] for r in native['results']])
        self.assertEqual(hashlib.sha256(encoded).hexdigest(), native['provenance']['request_sha256'])
        for name, digest in native['provenance']['sources'].items():
            archived = HERE / ('observations/source_snapshots/' + digest + '.gz')
            self.assertEqual(hashlib.sha256(gzip.decompress(archived.read_bytes())).hexdigest(),
                             digest, name)

    def test_every_public_station_passes_the_fitting_reserve(self):
        inputs = json.loads((HERE / 'fixtures/loft_refit_closest_stations_inputs.json').read_text())
        rows = json.loads((HERE / 'observations/loft_refit_closest_stations.json').read_text())['results']
        sources = json.loads((HERE / 'fixtures/loft_refit_join_modes.json').read_text())['operations']
        contexts = json.loads((HERE / 'observations/loft_refit_join_modes.json').read_text())['results']
        self.assertEqual(len(inputs['cases']), 21)
        for i, case in enumerate(inputs['cases']):
            self.assertEqual(len(case['targets']), 7)
            source = next(o for o in sources if o['id'] == case['source_id'])
            self.assertEqual(case['source'], source['curves'][0])
            context = next(r for r in contexts if r['id'] == case['source_id'])
            lookup = context['value']['refit_sources'][1]['numerical_clocks'][3]['batch_parameters']
            start, end = case['interval']
            for station, parameter in enumerate(case['parameters'], 1):
                f = (start + (end - start) * station / 8.) * 8.
                j = min(int(f), 7)
                expected = lookup[j] * (1. - (f - j)) + lookup[j + 1] * (f - j)
                self.assertAlmostEqual(parameter, expected, delta=1e-14)
            for row in rows[7 * i:7 * (i + 1)]:
                self.assertLess(row['value']['distance'], case['refit_tolerance'] * .5)


if __name__ == '__main__':
    unittest.main()
