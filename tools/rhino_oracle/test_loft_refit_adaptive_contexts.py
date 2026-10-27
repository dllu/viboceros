"""Immutable adaptive fitting records across document and SDK contexts."""
import gzip
import hashlib
import json
from pathlib import Path
import unittest
from .generate_loft_refit_adaptive_contexts import request

HERE = Path(__file__).resolve().parent
FAMILY = 'loft_refit_adaptive_contexts'


def records():
    return json.loads((HERE / ('observations/' + FAMILY + '.json')).read_text())


class AdaptiveContextTests(unittest.TestCase):
    def test_recipe_archive_and_source_state(self):
        encoded = (HERE / ('fixtures/' + FAMILY + '.json')).read_bytes()
        query = json.loads(encoded)
        native = records()
        self.assertEqual(query, request())
        self.assertEqual(len(query['operations']), 64)
        self.assertEqual([op['id'] for op in query['operations']],
                         [row['id'] for row in native['results']])
        self.assertEqual(hashlib.sha256(encoded).hexdigest(),
                         native['provenance']['request_sha256'])
        for name, digest in native['provenance']['sources'].items():
            archive = HERE / ('observations/source_snapshots/' + digest + '.gz')
            self.assertEqual(hashlib.sha256(gzip.decompress(archive.read_bytes())).hexdigest(),
                             digest, name)
        for row in native['results']:
            value = row['value']
            if 'before' in value:
                self.assertEqual(value['before'], value['after'])
            else:
                self.assertTrue(value['succeeded'])
                self.assertTrue(all(value['originals_present']))

    def test_styles_commands_and_curvature_sensitive_model_units(self):
        native = records()
        widths = [28, 44, 38, 20]
        variant_widths = [[28, 28, 28, 28, 41, 28],
                          [44, 44, 44, 44, 49, 34],
                          [38, 37, 38, 38, 43, 30],
                          [20, 20, 20, 20, 24, 20]]
        for shape, expected in enumerate(widths):
            rows = native['results'][shape * 16:(shape + 1) * 16]
            for i, row in enumerate(rows[:10]):
                value = row['value']
                surfaces = value['outputs'][0]['surfaces'] if i % 2 else value['surfaces']
                self.assertEqual(surfaces[0]['control_count'][1], expected)
                self.assertTrue(surfaces[0]['samples'])
            self.assertEqual([r['value']['surfaces'][0]['control_count'][1]
                              for r in rows[10:]], variant_widths[shape])


if __name__ == '__main__':
    unittest.main()
