"""Frozen command, mixed-profile, point-end and circular-clock holdouts."""
import gzip
import hashlib
import json
from pathlib import Path
import unittest
from .generate_loft_refit_adaptive_admission import request as admission
from .generate_loft_refit_arc_clocks import request as clocks

HERE = Path(__file__).resolve().parent


class AdaptiveAdmissionTests(unittest.TestCase):
    def test_recipes_archives_and_original_source_state(self):
        for family, recipe, count in [('loft_refit_adaptive_admission', admission, 48),
                                       ('loft_refit_arc_clocks', clocks, 4)]:
            with self.subTest(family=family):
                encoded = (HERE / ('fixtures/' + family + '.json')).read_bytes()
                query = json.loads(encoded)
                native = json.loads((HERE / ('observations/' + family + '.json')).read_text())
                self.assertEqual(query, recipe())
                self.assertEqual(len(query['operations']), count)
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

    def test_styles_and_independent_mixed_and_point_end_inputs(self):
        operations = admission()['operations']
        for i in range(0, 48, 12):
            self.assertEqual({op['style'] for op in operations[i:i + 10]},
                             {'normal', 'loose', 'tight', 'straight', 'uniform'})
            self.assertEqual([op['command'] for op in operations[i:i + 10]],
                             [False, True] * 5)
            self.assertEqual(operations[i + 10]['curves'][1]['degree'], 1)
            self.assertIn('start_point', operations[i + 11])
            self.assertIn('end_point', operations[i + 11])


if __name__ == '__main__':
    unittest.main()
