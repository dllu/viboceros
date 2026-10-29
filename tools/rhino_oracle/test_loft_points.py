"""Retained point-end API coverage, empty results, and native endpoint omissions."""
from tools.verify_provenance import verify_measurement
import gzip
import ast
import hashlib
import json
import math
import unittest
from pathlib import Path
from .generate_loft_points import request

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent


def honors_points(surface, operation):
    nu, nv = surface['control_count']
    for key, u in [('start_point', 0), ('end_point', nu - 1)]:
        if key not in operation:
            continue
        for v in range(nv):
            point = surface['control_points'][v * nu + u]['point']
            if any(abs(a-b) > 1e-9 for a, b in zip(point, operation[key])):
                return False
    return True


class LoftPointTests(unittest.TestCase):
    def test_native_worker_rejects_bad_points_before_accessing_rhino(self):
        tree = ast.parse((HERE / 'rhino_worker.py').read_text())
        function = next(node for node in tree.body if isinstance(node, ast.FunctionDef) and node.name == '_loft')
        namespace = {'math': math}
        exec(compile(ast.Module(body=[function], type_ignores=[]), 'native-loft-validation', 'exec'), namespace)
        for value in [True, [], [0., 1.], [0., 1., 2., 3.], [False, 0., 0.],
                      [0., float('nan'), 0.], [0., 0., float('inf')]]:
            for key in ['start_point', 'end_point']:
                with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                    namespace['_loft']({key: value}, 1)
        for option in ['command', 'closed']:
            with self.subTest(option=option), self.assertRaises(ValueError):
                namespace['_loft']({'start_point': [0., 0., 0.], option: True}, 1)

    def test_captured_recipes_sources_and_native_point_contract(self):
        counts = dict(surfaces=0, empty=0, ignored_points=0, uniform_empty=0)
        for family, more in [('loft_points', False), ('loft_points_more_sections', True)]:
            fixture = HERE / ('fixtures/' + family + '.json')
            query = json.loads(fixture.read_text())
            data = json.loads((HERE / ('observations/' + family + '.json')).read_text())
            self.assertEqual(query, request(more_sections=more))
            self.assertEqual(hashlib.sha256(fixture.read_bytes()).hexdigest(),
                             data['provenance']['request_sha256'])
            for name, sha in data['provenance']['sources'].items():
                archive = HERE / ('observations/source_snapshots/' + sha + '.gz')
                self.assertEqual(hashlib.sha256(gzip.decompress(archive.read_bytes())).hexdigest(), sha, name)
            self.assertEqual([o['id'] for o in query['operations']], [r['id'] for r in data['results']])
            for op, row in zip(query['operations'], data['results']):
                self.assertEqual(row['value']['before'], row['value']['after'])
                surfaces = row['value']['surfaces']
                if not surfaces:
                    counts['empty'] += 1
                    if op['style'] == 'uniform':
                        counts['uniform_empty'] += 1
                    continue
                self.assertEqual(len(surfaces), 1)
                surface = surfaces[0]
                self.assertEqual(len(surface['samples']), 289)
                if honors_points(surface, op):
                    counts['surfaces'] += 1
                else:
                    counts['ignored_points'] += 1
                    self.assertEqual(op['curves'][0]['degree'], 1)
                    self.assertEqual(len(op['curves']), 2)
                    self.assertIn('end_point', op)
                    self.assertGreater(max(abs(surface['samples'][-1][i] - op['end_point'][i]) for i in range(3)), 1.)
        self.assertEqual(counts, dict(surfaces=96, empty=50, ignored_points=4, uniform_empty=30))

    def test_qualification_source_bindings(self):
        record = json.loads((ROOT / 'docs/loft-point-ends.json').read_text())
        self.assertEqual(record['native_cases'], 150)
        self.assertEqual(record['matched_surfaces_per_backend'], 96)
        self.assertEqual(record['native_ignored_point_cases'], 4)
        self.assertIs(record['full_rhino_parity'], False)
        self.assertGreater(verify_measurement(record), 0)


if __name__ == '__main__':
    unittest.main()
