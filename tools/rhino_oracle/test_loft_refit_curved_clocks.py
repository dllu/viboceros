"""Frozen curved-source clocks, compatibility fitting and command holdouts."""
import gzip
import hashlib
import json
from pathlib import Path
import unittest
from .generate_loft_refit_curved_clocks import request as clocks
from .generate_loft_refit_curved_frames import request as frames
from .generate_loft_refit_projected_order import request as order
from .test_source_continuity import difference

HERE = Path(__file__).resolve().parent


def native(family):
    return json.loads((HERE / ('observations/' + family + '.json')).read_text())


class CurvedClockTests(unittest.TestCase):
    def test_recipes_archives_and_complete_coverage(self):
        for family, recipe, count in [('loft_refit_curved_clocks', clocks, 12),
                                     ('loft_refit_curved_frames', frames, 64),
                                     ('loft_refit_projected_order', order, 4)]:
            encoded = (HERE / ('fixtures/' + family + '.json')).read_bytes()
            query, recorded = json.loads(encoded), native(family)
            self.assertEqual(query, recipe())
            self.assertEqual(len(query['operations']), count)
            self.assertEqual([op['id'] for op in query['operations']],
                             [row['id'] for row in recorded['results']])
            self.assertEqual(hashlib.sha256(encoded).hexdigest(), recorded['provenance']['request_sha256'])
            for name, digest in recorded['provenance']['sources'].items():
                data = gzip.decompress((HERE / ('observations/source_snapshots/' + digest + '.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(data).hexdigest(), digest, name)

    def test_paired_fitting_transitions_differ_from_single_input_fitting(self):
        rows = native('loft_refit_curved_clocks')['results']
        paired = [len(row['value']['refit_sources'][1]['curves'][0]['control_points']) for row in rows]
        single = [len(row['value']['refit_sources'][0]['curves'][0]['control_points']) for row in rows]
        self.assertEqual(paired, [6] * 5 + [4, 8] + [4] * 5)
        self.assertEqual(single, [6] * 3 + [4] * 9)
        for row in rows:
            value = row['value']
            self.assertEqual(value['before'], value['after'])
            sources = value['refit_sources']
            for i in [0, 1]:
                self.assertLessEqual(difference(sources[i]['curves'], sources[i + 2]['curves']), 1e-9)

    def test_clocks_and_source_continuity_are_independent_of_requested_fit_tolerance(self):
        rows = native('loft_refit_curved_clocks')['results']
        for group in [rows[:6], rows[6:]]:
            expected = group[0]['value']['refit_sources'][1]['numerical_clocks']
            for row in group:
                source = row['value']['refit_sources'][1]
                self.assertEqual(source['numerical_clocks'], expected)
                self.assertEqual(source['source_structure']['discontinuities'],
                    dict(C1_continuous=[], G1_continuous=[], G2_continuous=[.5]))

    def test_native_initial_fits_preserve_sources_across_commands_and_frames(self):
        for row in native('loft_refit_curved_frames')['results']:
            value = row['value']
            if 'surfaces' in value:
                self.assertEqual(value['before'], value['after'])
                self.assertTrue(all(s['control_count'][1] == 4 for s in value['surfaces']))
            else:
                self.assertTrue(value['succeeded'])
                self.assertTrue(all(value['originals_present']))
                self.assertTrue(all(value['originals_selected']))
                self.assertEqual(len(value['outputs']), 1)
                self.assertTrue(value['outputs'][0]['valid'])

    def test_retraced_line_fitting_preserves_traversal_beyond_its_geometric_image(self):
        rows = native('loft_refit_projected_order')['results']
        self.assertEqual([row['value']['surfaces'][0]['control_count'][1] for row in rows],
                         [4, 4, 50, 34])
        for row in rows:
            self.assertEqual(row['value']['before'], row['value']['after'])


if __name__ == '__main__':
    unittest.main()
