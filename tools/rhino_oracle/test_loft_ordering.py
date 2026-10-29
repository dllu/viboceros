"""Native recipes and witnesses for open Loft ordering and traversal."""
from tools.verify_provenance import verify_measurement
import gzip
import hashlib
import json
from pathlib import Path
import unittest
from .generate_loft_ordering import request, line_request, holdout_request, spatial_request

HERE = Path(__file__).resolve().parent
FAMILIES = [
    ('loft_ordering_smoke', lambda: request(True), 8),
    ('loft_ordering', request, 180),
    ('loft_line_ordering', line_request, 120),
    ('loft_ordering_holdouts', holdout_request, 48),
    ('loft_ordering_spatial', spatial_request, 108),
]


def capture(family):
    return json.loads((HERE / ('observations/' + family + '.json')).read_text())


def ends(row):
    surface = row['value']['outputs'][0]['surfaces'][0]
    nu = surface['control_count'][0]
    return [surface['control_points'][i]['point'] for i in [0, nu - 1]]


class LoftOrderingTests(unittest.TestCase):
    def test_qualification_source_bindings(self):
        root = HERE.parents[1]
        record = json.loads((root / 'docs/loft-ordering-validation.json').read_text())
        self.assertEqual(record['normal_command_records'], 464)
        self.assertEqual(record['distinct_normal_command_recipes'], 456)
        self.assertFalse(record['full_rhino_parity'])
        self.assertFalse(record['continuous_error_certificate'])
        self.assertFalse(record['performance_qualified'])
        self.assertGreater(verify_measurement(record), 0)

    def test_native_recipes_coverage_source_state_and_frozen_producers(self):
        ids = []
        for family, recipe, count in FAMILIES:
            fixture = HERE / ('fixtures/' + family + '.json')
            query = json.loads(fixture.read_text())
            data = capture(family)
            self.assertEqual(query, recipe())
            self.assertEqual(len(query['operations']), count)
            self.assertEqual([o['id'] for o in query['operations']], [r['id'] for r in data['results']])
            self.assertEqual(hashlib.sha256(fixture.read_bytes()).hexdigest(),
                             data['provenance']['request_sha256'])
            for name, digest in data['provenance']['sources'].items():
                archive = HERE / ('observations/source_snapshots/' + digest + '.gz')
                self.assertEqual(hashlib.sha256(gzip.decompress(archive.read_bytes())).hexdigest(), digest, name)
            for op, row in zip(query['operations'], data['results']):
                ids.append(op['id'])
                value = row['value']
                self.assertTrue(value['succeeded'])
                self.assertEqual(value['originals_present'], [True] * len(op['curves']))
                self.assertEqual(value['originals_selected'], [True] * len(op['curves']))
                self.assertEqual(len(value['input_planes']), len(op['curves']))
                self.assertTrue(all(plane['planar'] for plane in value['input_planes']))
                self.assertEqual(len(value['outputs']), 1)
                for output in value['outputs']:
                    self.assertTrue(output['valid'])
                    self.assertFalse(output['selected'])
                    self.assertEqual(output['group_count'], 0)
                    self.assertEqual(len(output['face_reversed']), len(output['surfaces']))
                    self.assertTrue(all(len(s['samples']) == 289 for s in output['surfaces']))
        self.assertEqual(len(ids), 464)
        # The eight pilot recipes were captured again in the full matrix.
        self.assertEqual(len(set(ids)), 456)

    def test_native_line_witnesses_distinguish_midpoints_from_other_metrics(self):
        rows = {r['id']: r for family in ['loft_line_ordering', 'loft_ordering_holdouts']
                for r in capture(family)['results']}
        for name, expected in [
            ('loft-line-order-nested-0-012', [[-1., 0., 2.], [49., 0., 5.]]),
            ('loft-line-order-spread-0-012', [[100., 0., 2.], [0., 0., 5.]]),
            ('loft-order-holdout-rational_line-0-012', [[89., 0., 5.], [39., 0., 2.]]),
            ('loft-order-holdout-cubic_line-0-012', [[-1., 0., 2.], [49., 0., 5.]]),
        ]:
            self.assertEqual(ends(rows[name]), expected)

    def test_zero_area_inflected_profiles_retain_the_same_plane_sign_on_reversal(self):
        rows = {r['id']: r for r in capture('loft_ordering')['results']}
        for backwards in [0, 1]:
            row = rows['loft-order-symmetric-{}-2-201'.format(backwards)]
            self.assertTrue(all(p['normal'] == [0., 0., -1.] for p in row['value']['input_planes']))
            self.assertAlmostEqual(ends(row)[0][2], 5., places=12)
            self.assertAlmostEqual(ends(row)[1][2], 0., places=12)


if __name__ == '__main__':
    unittest.main()
