"""Analytic images and provenance for portable/native mesh Pull commands."""
import json
import math
from pathlib import Path
import unittest
from tools.verify_provenance import verify

ROOT = Path(__file__).resolve().parents[2]


def read(path):
    return json.loads((ROOT / path).read_text())


class MeshPullTests(unittest.TestCase):
    def test_retained_outputs_match_analytic_chord_images(self):
        request = read('tools/rhino_oracle/fixtures/pull_curve_mesh.json')
        tolerance = request['tolerance']['absolute']
        expected = {'flat-boundary': 8., 'folded-faces': 1. + math.sqrt(2.),
                    'collapsed': 0., 'source-reversal': 7.}
        payloads = []
        for backend in ('portable', 'native'):
            rows = read('tools/rhino_oracle/observations/pull_curve_mesh_viboceros_' + backend + '.json')['results']
            self.assertEqual([r['id'] for r in rows], [op['id'] for op in request['operations']])
            payloads.append([row['value'] for row in rows])
            for row in rows:
                with self.subTest(backend=backend, operation=row['id']):
                    curves = row['value']['curves']
                    length = expected[row['id']]
                    self.assertEqual(len(curves), int(length > 0.))
                    self.assertLessEqual(abs(sum(c['length'] for c in curves) - length), tolerance)
                    for curve in curves:
                        self.assertFalse(curve['closed'])
                        self.assertEqual(len(curve['samples']), 33)
                        for x, y, z in curve['samples']:
                            if row['id'] == 'folded-faces':
                                self.assertLessEqual(abs(y) + abs(z + max(x, 0.)), tolerance)
                                self.assertTrue(-1. - tolerance <= x <= 1. + tolerance)
                            else:
                                target_y = 1. if row['id'] == 'flat-boundary' else 0.
                                self.assertLessEqual(abs(y - target_y) + abs(z), tolerance)
        self.assertEqual(payloads[0], payloads[1])

    def test_qualification_preserves_sources_and_explicit_parity_limits(self):
        record = read('docs/mesh-pull-qualification.json')
        self.assertTrue(record['mesh_curve_pull_command_implemented'])
        self.assertFalse(record['full_rhino_parity'])
        self.assertFalse(record['performance_qualified'])
        self.assertEqual(record['fresh_rhino_observations'], 0)
        self.assertGreaterEqual(verify(record), 15)
