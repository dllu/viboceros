"""Frozen adaptive curved Loft fits across commands and source charts."""
import gzip
import hashlib
import json
from pathlib import Path
import unittest

from .generate_loft_refit_adaptive_curved import request
from .generate_loft_refit_adaptive_shapes import request as shapes
from .generate_loft_refit_adaptive_shape_clocks import request as shape_clocks

HERE = Path(__file__).resolve().parent
FAMILY = 'loft_refit_adaptive_curved'


class AdaptiveCurvedTests(unittest.TestCase):
    def captured(self, family, recipe, count):
        encoded = (HERE / ('fixtures/' + family + '.json')).read_bytes()
        query = json.loads(encoded)
        native = json.loads((HERE / ('observations/' + family + '.json')).read_text())
        self.assertEqual(query, recipe())
        self.assertEqual(len(query['operations']), count)
        self.assertEqual([op['id'] for op in query['operations']],
                         [row['id'] for row in native['results']])
        provenance = native['provenance']
        self.assertEqual(hashlib.sha256(encoded).hexdigest(), provenance['request_sha256'])
        for name, digest in provenance['sources'].items():
            path = HERE / ('observations/source_snapshots/' + digest + '.gz')
            self.assertEqual(hashlib.sha256(gzip.decompress(path.read_bytes())).hexdigest(),
                             digest, name)
        return query, native

    def test_recipe_and_source_archives_are_complete(self):
        self.captured(FAMILY, request, 48)

    def test_native_surface_topology_and_document_state(self):
        query, native = self.captured(FAMILY, request, 48)
        commands = 0
        for op, row in zip(query['operations'], native['results']):
            value = row['value']
            if op.get('command'):
                commands += 1
                self.assertTrue(value['succeeded'])
                for key in ['originals_present', 'originals_selected']:
                    self.assertEqual(value[key], [True] * len(op['curves']))
                self.assertEqual(len(value['outputs']), 1)
                self.assertTrue(value['outputs'][0]['valid'])
                surfaces = value['outputs'][0]['surfaces']
            else:
                self.assertEqual(value['before'], value['after'])
                surfaces = value['surfaces']
            self.assertEqual(len(surfaces), 1)
            kind = op['id'].removeprefix('loft-refit-adaptive-curved-').split('-')[0]
            self.assertEqual(surfaces[0]['control_count'][1],
                             {'equal': 6, 'unequal': 8, 'tight': 10}[kind])
        self.assertEqual(commands, 15)

    def test_independent_shapes_retain_complete_native_definitions(self):
        _, native = self.captured('loft_refit_adaptive_shapes', shapes, 12)
        self.assertEqual([row['value'][0]['control_count'][1] for row in native['results']],
                         [35, 31, 19, 44, 37, 18, 38, 36, 22, 21, 19, 14])
        for row in native['results']:
            surface = row['value'][0]
            self.assertEqual(len(surface['control_points']),
                             surface['control_count'][0] * surface['control_count'][1])

    def test_independent_shape_clocks_preserve_the_original_sources(self):
        _, native = self.captured('loft_refit_adaptive_shape_clocks', shape_clocks, 4)
        for row, single, paired in zip(native['results'], [22, 24, 24, 16], [35, 44, 38, 21]):
            value = row['value']
            self.assertEqual(value['before'], value['after'])
            for i, count in enumerate([single, paired]):
                inspected = value['refit_sources'][i]
                self.assertEqual(len(inspected['curves'][0]['control_points']), count)
                self.assertEqual(inspected['source_structure']['discontinuities'],
                                 dict(C1_continuous=[], G1_continuous=[], G2_continuous=[]))
                for clock in inspected['numerical_clocks']:
                    batch = clock['batch_parameters']
                    self.assertEqual(len(batch), 9)
                    self.assertEqual((batch[0], batch[-1]), (0., 1.))
                    self.assertTrue(all(a < b for a, b in zip(batch, batch[1:])))


if __name__ == '__main__':
    unittest.main()
