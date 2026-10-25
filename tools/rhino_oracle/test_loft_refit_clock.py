"""Native profile-set contracts and unclamped straight-profile holdouts."""
import gzip
import hashlib
import json
import math
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import Mock
from .generate_loft_refit_clock import request as clock_request
from .generate_loft_refit_sources import request as sources_request
from .generate_loft_refit_line_inputs import request as lines_request
from .loft_refit_clock_probe import inspect

HERE = Path(__file__).resolve().parent


def captured(family):
    return json.loads((HERE / ('observations/' + family + '.json')).read_text())


class RefitClockTests(unittest.TestCase):
    def test_native_recipes_ids_archives_and_source_preservation(self):
        for family, recipe, count in [('loft_refit_clock', clock_request, 21),
                ('loft_refit_sources', sources_request, 9),
                ('loft_refit_line_inputs', lines_request, 90)]:
            path = HERE / ('fixtures/' + family + '.json')
            query = json.loads(path.read_text())
            data = captured(family)
            self.assertEqual(query, recipe())
            self.assertEqual(len(query['operations']), count)
            self.assertEqual([o['id'] for o in query['operations']], [r['id'] for r in data['results']])
            self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(), data['provenance']['request_sha256'])
            for name, digest in data['provenance']['sources'].items():
                archive = HERE / ('observations/source_snapshots/' + digest + '.gz')
                self.assertEqual(hashlib.sha256(gzip.decompress(archive.read_bytes())).hexdigest(), digest, name)
            for row in data['results']:
                self.assertEqual(row['value']['before'], row['value']['after'])

    def test_paired_compatible_curves_reproduce_native_loft_sections(self):
        for row in captured('loft_refit_sources')['results']:
            value = row['value']
            surface = value['surfaces'][0]
            nu, nv = surface['control_count']
            prepared = next(r['curves'] for r in value['refit_sources']
                            if r.get('simplified') is False and r['source_count'] == 2)
            self.assertEqual(len(prepared), 2)
            self.assertEqual(nu, 2)
            for i, curve in enumerate(prepared):
                self.assertEqual(curve['degree'], surface['degree'][1])
                self.assertEqual(len(curve['control_points']), nv)
                for j, point in enumerate(curve['control_points']):
                    self.assertEqual(point, surface['control_points'][j * nu + i])
                normal = lambda knots: [(t - knots[0]) / (knots[-1] - knots[0]) for t in knots]
                self.assertLessEqual(max(abs(a - b) for a, b in zip(normal(curve['knots']),
                    normal(surface['knots_v']))), 2e-16)

    def test_single_curve_fitting_does_not_reproduce_paired_refit(self):
        differences = 0
        for row in captured('loft_refit_sources')['results']:
            rows = row['value']['refit_sources']
            single = rows[0]['curves'][0]['control_points']
            pair = rows[1]['curves'][0]['control_points']
            if len(single) != len(pair) or max(math.dist(a['point'], b['point'])
                    for a, b in zip(single, pair)) > 1e-9:
                differences += 1
        self.assertEqual(differences, 8)

    def test_line_holdouts_have_distinct_sources_and_polynomial_cubic_sections(self):
        query = lines_request()
        for op, row in zip(query['operations'], captured('loft_refit_line_inputs')['results']):
            self.assertEqual([c['control_points'][0]['point'][2] for c in op['curves']], [0., 2., 4.])
            self.assertEqual(len({json.dumps(c, sort_keys=True) for c in op['curves']}), 3)
            self.assertEqual(len(row['value']['surfaces']), 1)
            surface = row['value']['surfaces'][0]
            self.assertEqual((surface['degree'][1], surface['control_count'][1]), (3, 4))
            self.assertTrue(all(c['weight'] == 1. for c in surface['control_points']))

    def test_clock_probe_disposes_private_inputs_and_results_on_definition_failure(self):
        source, fitted = Mock(), Mock()
        curve = Mock()
        curve.DuplicateCurve.return_value = source
        curve.NormalizedLengthParameter.side_effect = lambda f, a: (True, f)
        curve.PointAt.return_value = SimpleNamespace(X=0., Y=0., Z=0.)
        curve.GetLength.return_value = 1.
        factory = Mock(return_value=[fitted])
        geometry = SimpleNamespace(NurbsCurve=SimpleNamespace(MakeCompatible=factory),
            Point3d=SimpleNamespace(Unset=None))
        with self.assertRaisesRegex(ValueError, 'definition'):
            inspect(curve, .001, .01, geometry, Mock(side_effect=ValueError('definition')))
        source.Dispose.assert_called_once_with()
        fitted.Dispose.assert_called_once_with()
        curve.Dispose.assert_not_called()


if __name__ == '__main__':
    unittest.main()
