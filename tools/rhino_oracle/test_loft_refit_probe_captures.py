"""Frozen evidence separates knot removal, angular policy and profile count."""
import gzip
import hashlib
import json
import math
from pathlib import Path
import unittest
from .generate_loft_refit_join_knots import request as knots
from .generate_loft_refit_adaptive_preparation import request as document_angle
from .generate_loft_refit_adaptive_default_angle import request as default_angle
from .generate_loft_refit_matched_probe_angles import request as matched_angles

HERE = Path(__file__).resolve().parent


def captured(family):
    return (json.loads((HERE / ('fixtures/' + family + '.json')).read_text()),
            json.loads((HERE / ('observations/' + family + '.json')).read_text()))


def paired(value):
    frame = next(f for f in value['refit_frames'] if f['variant'] == 'coincident')
    return next(m for m in frame['methods'] if m['method'] == 2)['curves'][0]


class RefitProbeCaptureTests(unittest.TestCase):
    def test_recipes_ids_and_archived_sources(self):
        for family, recipe, count in [
                ('loft_refit_join_knots', knots, 4),
                ('loft_refit_adaptive_preparation', document_angle, 4),
                ('loft_refit_adaptive_default_angle', default_angle, 4),
                ('loft_refit_matched_probe_angles', matched_angles, 10)]:
            query, native = captured(family)
            self.assertEqual(query, recipe())
            self.assertEqual(len(query['operations']), count)
            self.assertEqual([o['id'] for o in query['operations']],
                             [r['id'] for r in native['results']])
            encoded = (HERE / ('fixtures/' + family + '.json')).read_bytes()
            self.assertEqual(hashlib.sha256(encoded).hexdigest(),
                             native['provenance']['request_sha256'])
            for name, digest in native['provenance']['sources'].items():
                content = gzip.decompress((HERE / ('observations/source_snapshots/'
                                                   + digest + '.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(content).hexdigest(), digest, name)
            if family != 'loft_refit_join_knots':
                for row in native['results']:
                    self.assertEqual(row['value']['before'], row['value']['after'])

    def test_corrected_inspections_retain_original_loft_geometry_and_explicit_angles(self):
        query, native = captured('loft_refit_matched_probe_angles')
        _, old = captured('loft_refit_adaptive_lines')
        original = {r['id'].removeprefix('loft-refit-adaptive-'): r['value']['surfaces']
                    for r in old['results']}
        for op, row in zip(query['operations'], native['results']):
            value = row['value']
            self.assertEqual(value['refit_probe_angle_tolerance'],
                             op.get('refit_angle_tolerance', math.pi / 180.))
            if '-small-' not in op['id']:
                suffix = op['id'].split('-probe-angles-', 1)[1].split('-', 1)[1]
                self.assertEqual(value['surfaces'], original[suffix])
            surface = value['surfaces'][0]
            prepared = paired(value)
            self.assertEqual(surface['degree'][1], prepared['degree'])
            self.assertEqual(surface['control_points'][::surface['control_count'][0]],
                             prepared['control_points'])
            # C0 region matching can change knot charts after public preparation;
            # equal controls alone do not establish global parameter equality.
            self.assertEqual(len(surface['knots_v']), len(prepared['knots']))
        for implicit, explicit in zip(native['results'][:4], native['results'][4:8]):
            self.assertEqual(implicit['value'], explicit['value'])

    def test_document_angle_previously_changed_the_independent_comparison(self):
        _, old = captured('loft_refit_adaptive_preparation')
        _, fixed = captured('loft_refit_matched_probe_angles')
        for before, after in zip(old['results'][:2], fixed['results'][:2]):
            self.assertEqual(before['value']['surfaces'], after['value']['surfaces'])
            self.assertEqual(len(paired(before['value'])['control_points']), 7)
            self.assertIn(len(paired(after['value'])['control_points']), [8, 10])
        # Equal coincident profiles retain the multi-input branch even when
        # both independent queries use the correct one-degree fitting angle.
        for row in fixed['results'][:4]:
            value = row['value']
            single = next(f for f in value['refit_frames'] if f['variant'] == 'single')
            single = next(m for m in single['methods'] if m['method'] == 2)['curves'][0]
            self.assertNotEqual(single, paired(value))


if __name__ == '__main__':
    unittest.main()
