"""Retain Loft Rebuild recipes, public producer sources and approximation limits."""
import ast
import gzip
import hashlib
import json
import math
from pathlib import Path
import unittest
from .generate_loft_rebuild import request

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]


class LoftRebuildTests(unittest.TestCase):
    def test_captured_recipes_and_frozen_producers(self):
        for family, kwargs, count in [
            ('loft_rebuild_smoke', dict(smoke=True), 8),
            ('loft_rebuild', {}, 390),
            ('loft_rebuild_bounds', dict(bounds=True), 35),
            ('loft_rebuild_commands', dict(commands=True), 140),
        ]:
            fixture = HERE / ('fixtures/' + family + '.json')
            query = json.loads(fixture.read_text())
            data = json.loads((HERE / ('observations/' + family + '.json')).read_text())
            self.assertEqual(query, request(**kwargs))
            self.assertEqual(len(query['operations']), count)
            self.assertEqual([o['id'] for o in query['operations']], [r['id'] for r in data['results']])
            self.assertEqual(hashlib.sha256(fixture.read_bytes()).hexdigest(), data['provenance']['request_sha256'])
            for name, digest in data['provenance']['sources'].items():
                archive = HERE / ('observations/source_snapshots/' + digest + '.gz')
                self.assertEqual(hashlib.sha256(gzip.decompress(archive.read_bytes())).hexdigest(), digest, name)

    def test_native_api_purity_empty_results_and_closed_seam_counts(self):
        surfaces = empty = 0
        for family in ['loft_rebuild_smoke', 'loft_rebuild', 'loft_rebuild_bounds']:
            query = json.loads((HERE / ('fixtures/' + family + '.json')).read_text())
            data = json.loads((HERE / ('observations/' + family + '.json')).read_text())
            for op, row in zip(query['operations'], data['results']):
                value = row['value']
                self.assertEqual(value['before'], value['after'])
                if not value['surfaces']:
                    self.assertEqual(op['style'], 'uniform')
                    self.assertTrue(op.get('start_point') or op.get('end_point'))
                    empty += 1
                    continue
                self.assertEqual(len(value['surfaces']), 1)
                surface = value['surfaces'][0]
                self.assertEqual(len(surface['samples']), 289)
                count = op['rebuild_point_count']
                source = op['curves'][0]
                closed = source['control_points'][0]['point'] == source['control_points'][-1]['point']
                if count >= (3 if closed else 2):
                    self.assertEqual(surface['degree'][1], min(3, count - 1))
                    self.assertEqual(surface['control_count'][1], count + int(closed))
                    self.assertTrue(all(cp['weight'] == 1. for cp in surface['control_points']))
                surfaces += 1
        self.assertEqual((surfaces, empty), (397, 36))

    def test_open_loft_sections_equal_independent_untangented_curve_rebuilds(self):
        query = json.loads((HERE / 'fixtures/loft_rebuild.json').read_text())
        data = json.loads((HERE / 'observations/loft_rebuild.json').read_text())
        count = 0
        for op, row in zip(query['operations'], data['results']):
            source = op['curves'][0]
            if (op['rebuild_point_count'] < 4 or op.get('start_point') or op.get('end_point')
                    or source['control_points'][0]['point'] == source['control_points'][-1]['point']):
                continue
            surface = row['value']['surfaces'][0]
            nu, nv = surface['control_count']
            expected = row['value']['rebuilt'][0][0]['control_points']
            self.assertEqual(len(expected), nv)
            for v in range(nv):
                for a, b in zip(surface['control_points'][v * nu]['point'], expected[v]['point']):
                    self.assertLess(abs(a - b), 1e-12)
            count += 1
        self.assertEqual(count, 45)

    def test_native_clamped_circle_control_exposes_the_strict_numerical_gap(self):
        rows = json.loads((HERE / 'observations/loft_rebuild_smoke.json').read_text())['results']
        row = next(r for r in rows if r['id'] == 'loft-rebuild-circle-4-normal-0')
        surface = row['value']['surfaces'][0]
        nu, _ = surface['control_count']
        # Cubic Greville interpolation through exact unit-circle arc fractions
        # gives x=25/24. Native's sampled curve differs at this control.
        error = abs(surface['control_points'][nu]['point'][0] - 25. / 24.)
        self.assertGreater(error, 1e-9)
        self.assertLess(error, 1e-7)

    def test_native_worker_rejects_invalid_counts_before_accessing_rhino(self):
        tree = ast.parse((HERE / 'rhino_worker.py').read_text())
        function = next(n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == '_loft')
        scope = {'math': math}
        exec(compile(ast.Module(body=[function], type_ignores=[]), 'loft-rebuild-validation', 'exec'), scope)
        for count in [True, -1, 513, 1.2, '6', float('nan')]:
            with self.subTest(count=count), self.assertRaises(ValueError):
                scope['_loft'](dict(rebuild_point_count=count), 1)


if __name__ == '__main__':
    unittest.main()
