"""Immutable native references for the straight-profile Refit implementation."""
import ast
import gzip
import hashlib
import json
import math
from pathlib import Path
import unittest
from .generate_loft_refit_lines import request as lines
from .generate_loft_refit_line_ends import request as ends
from .generate_loft_refit_policy import request as policy

HERE = Path(__file__).resolve().parent


class StraightRefitTests(unittest.TestCase):
    def test_native_recipes_coverage_and_content_addressed_producers(self):
        for family, recipe, count in [('loft_refit_policy', policy, 78),
                ('loft_refit_lines', lines, 83), ('loft_refit_line_ends', ends, 17)]:
            fixture = HERE / ('fixtures/' + family + '.json')
            query = json.loads(fixture.read_text())
            data = json.loads((HERE / ('observations/' + family + '.json')).read_text())
            self.assertEqual(query, recipe())
            self.assertEqual(len(query['operations']), count)
            self.assertEqual([o['id'] for o in query['operations']], [r['id'] for r in data['results']])
            self.assertEqual(hashlib.sha256(fixture.read_bytes()).hexdigest(), data['provenance']['request_sha256'])
            for name, digest in data['provenance']['sources'].items():
                source = gzip.decompress((HERE / ('observations/source_snapshots/' + digest + '.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(source).hexdigest(), digest, name)

    def test_native_straight_profiles_always_become_cubic_lines_without_editing_sources(self):
        data = json.loads((HERE / 'observations/loft_refit_lines.json').read_text())
        for row in data['results']:
            value = row['value']
            self.assertEqual(value['before'], value['after'])
            self.assertEqual(len(value['surfaces']), 1)
            surface = value['surfaces'][0]
            self.assertEqual(surface['degree'][1], 3)
            self.assertEqual(surface['control_count'][1], 4)
            self.assertTrue(all(c['weight'] == 1. for c in surface['control_points']))
            self.assertEqual(len(surface['samples']), 289)

    def test_native_worker_validates_explicit_refit_angle_before_accessing_rhino(self):
        tree = ast.parse((HERE / 'rhino_worker.py').read_text())
        function = next(n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == '_loft')
        scope = {'math': math}
        exec(compile(ast.Module(body=[function], type_ignores=[]), 'refit-angle', 'exec'), scope)
        for angle in [True, -1., math.pi + .1, '0.1', float('nan'), float('inf')]:
            with self.subTest(angle=angle), self.assertRaisesRegex(ValueError, 'angle tolerance'):
                scope['_loft'](dict(refit_tolerance=.001, refit_angle_tolerance=angle), 1)
        with self.assertRaisesRegex(ValueError, 'angle tolerance'):
            scope['_loft'](dict(refit_angle_tolerance=0.), 1)


if __name__ == '__main__':
    unittest.main()
