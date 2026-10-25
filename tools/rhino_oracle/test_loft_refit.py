"""Retain native Refit evidence without treating ordinary Loft as a fitted result."""
import ast
from collections import Counter
import gzip
import hashlib
import json
import math
from pathlib import Path
import unittest
from .generate_loft_refit import request

HERE = Path(__file__).resolve().parent


def captured(family):
    return (json.loads((HERE / ('fixtures/' + family + '.json')).read_text()),
            json.loads((HERE / ('observations/' + family + '.json')).read_text()))


class LoftRefitTests(unittest.TestCase):
    def test_captured_recipes_ids_requests_and_archived_producers(self):
        for family, smoke, count in [('loft_refit_smoke', True, 16), ('loft_refit', False, 100)]:
            query, data = captured(family)
            self.assertEqual(query, request(smoke))
            self.assertEqual(len(query['operations']), count)
            self.assertEqual([o['id'] for o in query['operations']], [r['id'] for r in data['results']])
            fixture = HERE / ('fixtures/' + family + '.json')
            self.assertEqual(hashlib.sha256(fixture.read_bytes()).hexdigest(), data['provenance']['request_sha256'])
            for name, digest in data['provenance']['sources'].items():
                archive = HERE / ('observations/source_snapshots/' + digest + '.gz')
                self.assertEqual(hashlib.sha256(gzip.decompress(archive.read_bytes())).hexdigest(), digest, name)

    def test_native_refit_is_polynomial_cubic_and_retains_all_sources(self):
        query, data = captured('loft_refit')
        fitted = disabled = 0
        for op, row in zip(query['operations'], data['results']):
            value = row['value']
            self.assertEqual(value['before'], value['after'])
            self.assertEqual(len(value['surfaces']), 1)
            surface = value['surfaces'][0]
            self.assertEqual(len(surface['samples']), 289)
            if op['refit_tolerance'] > 0.:
                self.assertEqual(surface['degree'][1], 3)
                self.assertTrue(all(p['weight'] == 1. for p in surface['control_points']))
                fitted += 1
            else:
                self.assertEqual(surface['degree'][1], op['curves'][0]['degree'])
                disabled += 1
        self.assertEqual((fitted, disabled), (75, 25))

    def test_zero_refit_equals_native_zero_rebuild(self):
        query, data = captured('loft_refit')
        _, old = captured('loft_rebuild')
        expected = {r['id']: r['value'] for r in old['results']}
        count = 0
        for op, row in zip(query['operations'], data['results']):
            if op['refit_tolerance'] != 0.:
                continue
            shape = op['id'].split('-')[2]
            key = 'loft-rebuild-{}-0-{}-0'.format(shape, op['style'])
            self.assertEqual(row['value'], expected[key])
            count += 1
        self.assertEqual(count, 25)

    def test_standalone_curve_fit_does_not_explain_smooth_refit_knot_multiplicities(self):
        query, data = captured('loft_refit')
        count = 0
        for op, row in zip(query['operations'], data['results']):
            if op['id'] != 'loft-refit-quarter-0.001-straight':
                continue
            value = row['value']
            for fit in value['refitted']:
                self.assertTrue(all(n == 1 for n in Counter(fit['knots'][4:-4]).values()))
            loft = value['surfaces'][0]
            self.assertGreater(max(Counter(loft['knots_v'][4:-4]).values()), 1)
            count += 1
        self.assertEqual(count, 1)

    def test_independent_cubic_bezier_diagnostics_preserve_evidence_and_sources(self):
        from .generate_loft_refit_diagnostics import request as diagnostic_request
        query, data = captured('loft_refit_diagnostics')
        self.assertEqual(query, diagnostic_request())
        self.assertEqual(len(query['operations']), 45)
        self.assertEqual([o['id'] for o in query['operations']], [r['id'] for r in data['results']])
        fixture = HERE / 'fixtures/loft_refit_diagnostics.json'
        self.assertEqual(hashlib.sha256(fixture.read_bytes()).hexdigest(), data['provenance']['request_sha256'])
        for name, digest in data['provenance']['sources'].items():
            archive = HERE / ('observations/source_snapshots/' + digest + '.gz')
            self.assertEqual(hashlib.sha256(gzip.decompress(archive.read_bytes())).hexdigest(), digest, name)
        for row in data['results']:
            value = row['value']
            self.assertEqual(value['before'], value['after'])
            self.assertEqual(len(value['refit_beziers']), 2)
            for pieces in value['refit_beziers']:
                self.assertTrue(pieces)
                for piece in pieces:
                    self.assertEqual(piece['degree'], 3)
                    self.assertEqual(len(piece['control_points']), 4)
                    self.assertTrue(all(p['weight'] == 1. for p in piece['control_points']))
        # Equal translated profiles eliminate common-basis union as an explanation.
        rows = {r['id']: r['value'] for r in data['results']}
        witness = rows['loft-refit-quarter-0.001-straight-profile-0']
        self.assertEqual(len(set(witness['surfaces'][0]['knots_v'])) - 1, 2)
        self.assertEqual(len(witness['refit_beziers'][0]), 4)
        native_handle = witness['surfaces'][0]['control_points'][2]['point']
        bezier_handle = witness['refit_beziers'][0][0]['control_points'][1]['point']
        self.assertGreater(math.dist(native_handle, bezier_handle), 0.2)

    def test_native_worker_rejects_invalid_tolerance_and_conflicts_before_rhino(self):
        tree = ast.parse((HERE / 'rhino_worker.py').read_text())
        function = next(n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == '_loft')
        scope = {'math': math}
        exec(compile(ast.Module(body=[function], type_ignores=[]), 'refit-validation', 'exec'), scope)
        for tolerance in [True, -1., '0.01', [], float('nan'), float('inf')]:
            with self.subTest(tolerance=tolerance), self.assertRaises(ValueError):
                scope['_loft'](dict(refit_tolerance=tolerance), 1)
        for rebuild in [0, 6]:
            with self.subTest(rebuild=rebuild), self.assertRaisesRegex(ValueError, 'conflicting'):
                scope['_loft'](dict(refit_tolerance=0., rebuild_point_count=rebuild), 1)


if __name__ == '__main__':
    unittest.main()
