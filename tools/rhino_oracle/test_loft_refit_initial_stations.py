"""Frozen measurements for initial multispan fits and public deviation."""
import gzip
import hashlib
import json
from pathlib import Path
import unittest
from .generate_loft_refit_deviation import request as deviations
from .generate_loft_refit_initial_stations import request as initial_stations
from .generate_loft_refit_initial_holdouts import request as curved_holdouts

HERE = Path(__file__).resolve().parent


def native(family):
    return json.loads((HERE / ('observations/' + family + '.json')).read_text())


class InitialStationTests(unittest.TestCase):
    def test_recipes_archives_and_order(self):
        for family, recipe, count in [('loft_refit_deviation', deviations, 52),
                                     ('loft_refit_initial_stations', initial_stations, 50),
                                     ('loft_refit_initial_holdouts', curved_holdouts, 24)]:
            encoded = (HERE / ('fixtures/' + family + '.json')).read_bytes()
            query, measured = json.loads(encoded), native(family)
            self.assertEqual(query, recipe())
            self.assertEqual(len(query['operations']), count)
            self.assertEqual([op['id'] for op in query['operations']],
                             [row['id'] for row in measured['results']])
            self.assertEqual(hashlib.sha256(encoded).hexdigest(),
                             measured['provenance']['request_sha256'])
            for name, digest in measured['provenance']['sources'].items():
                archived = gzip.decompress((HERE / ('observations/source_snapshots/'
                                                     + digest + '.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(archived).hexdigest(), digest, name)

    def test_public_maximum_distance_is_invariant_across_clocks_and_reversal(self):
        rows = native('loft_refit_deviation')['results'][:48]
        maxima = [2.96296296276909e-6, 1.481481481532693e-6,
                  7.40740740618199e-7, 3.703703704578514e-7,
                  1.8518518508077672e-7, 9.259259268853712e-8]
        for row in rows:
            index = int(row['id'].rsplit('-', 1)[1])
            value = row['value']
            self.assertTrue(value['sdk']['created'])
            self.assertAlmostEqual(value['sdk']['maximum']['distance'], maxima[index], delta=1e-15)
            self.assertEqual(value['sdk']['minimum']['distance'], 0.)
            self.assertTrue(value['inputs_retained'])

    def test_public_deviation_detects_features_between_global_length_stations(self):
        rows = native('loft_refit_deviation')['results'][48:]
        self.assertEqual(len(rows), 4)
        for row in rows:
            self.assertAlmostEqual(row['value']['sdk']['maximum']['distance'], 1e-6, delta=1e-15)
            self.assertEqual(row['value']['sdk']['minimum']['distance'], 0.)

    def test_native_initial_fits_preserve_api_sources_and_command_selection(self):
        rows = native('loft_refit_initial_stations')['results']
        for row in rows:
            value = row['value']
            if row['id'].endswith('-sdk'):
                self.assertEqual(value['before'], value['after'])
                self.assertTrue(value['surfaces'])
                self.assertTrue(all(s['control_count'][1] == 4 for s in value['surfaces']))
            else:
                self.assertTrue(value['succeeded'])
                self.assertTrue(all(value['originals_present']))
                self.assertTrue(all(value['originals_selected']))
                self.assertEqual(len(value['outputs']), 1)
                self.assertTrue(value['outputs'][0]['valid'])

    def test_curved_holdouts_preserve_sources_and_expose_subdivision_transitions(self):
        rows = native('loft_refit_initial_holdouts')['results']
        self.assertEqual(len(rows), 24)
        for row in rows:
            self.assertEqual(row['value']['before'], row['value']['after'])
        self.assertEqual(sum(row['value']['surfaces'][0]['control_count'][1] == 4
                             for row in rows), 18)


if __name__ == '__main__':
    unittest.main()
