"""Frozen native references for the common smooth handle correction."""
import gzip
import hashlib
import json
from pathlib import Path
import unittest
from .generate_loft_refit_adjustment_queries import request as queries
from .generate_loft_refit_adjustment import request as adjustment
from .generate_loft_refit_adjustment_fit_clock import request as fit_clock
from .generate_loft_refit_adjustment_ellipses import request as ellipses
from .generate_loft_refit_adjustment_local import request as local
from .generate_loft_refit_adjustment_commands import request as commands

HERE = Path(__file__).resolve().parent


class SmoothRefitAdjustmentTests(unittest.TestCase):
    def test_recipes_archives_and_native_source_purity(self):
        for family, recipe, count in [('loft_refit_adjustment_queries', queries, 18),
                ('loft_refit_adjustment', adjustment, 28),
                ('loft_refit_adjustment_fit_clock', fit_clock, 56),
                ('loft_refit_adjustment_ellipses', ellipses, 15),
                ('loft_refit_adjustment_local', local, 45),
                ('loft_refit_adjustment_commands', commands, 10)]:
            encoded = (HERE / ('fixtures/' + family + '.json')).read_bytes()
            query = json.loads(encoded)
            native = json.loads((HERE / ('observations/' + family + '.json')).read_text())
            self.assertEqual(query, recipe())
            self.assertEqual(len(query['operations']), count)
            self.assertEqual([o['id'] for o in query['operations']], [r['id'] for r in native['results']])
            self.assertEqual(hashlib.sha256(encoded).hexdigest(), native['provenance']['request_sha256'])
            for name, digest in native['provenance']['sources'].items():
                source = gzip.decompress((HERE / ('observations/source_snapshots/' + digest + '.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(source).hexdigest(), digest, name)
            if family not in ['loft_refit_adjustment_queries', 'loft_refit_adjustment_commands']:
                for row in native['results']:
                    self.assertEqual(row['value']['before'], row['value']['after'])

    def test_candidate_rationality_retains_the_public_batch_clock(self):
        data = json.loads((HERE / 'observations/loft_refit_adjustment_fit_clock.json').read_text())
        self.assertEqual(len(data['results']), 56)
        for a, b in zip(data['results'][::2], data['results'][1::2]):
            a, b = a['value']['refit_adjustment'], b['value']['refit_adjustment']
            self.assertTrue(a['candidate_is_rational'])
            self.assertFalse(b['candidate_is_rational'])
            self.assertEqual(a['parameters'], b['parameters'])
            self.assertEqual(a['points'], b['points'])
            self.assertAlmostEqual(a['midpoint_closest_parameter'], b['midpoint_closest_parameter'], delta=1e-12)

    def test_command_capture_retains_sources_and_straight_loft_face_splitting(self):
        data = json.loads((HERE / 'observations/loft_refit_adjustment_commands.json').read_text())
        for row in data['results']:
            value = row['value']
            self.assertTrue(value['succeeded'])
            self.assertTrue(all(value['originals_present']))
            self.assertTrue(all(value['originals_selected']))
            self.assertEqual(len(value['outputs']), 1)
            output = value['outputs'][0]
            self.assertTrue(output['valid'])
            self.assertFalse(output['selected'])
            faces = 2 if row['id'].endswith('-quarter-straight') else 1
            self.assertEqual(len(output['surfaces']), faces)


if __name__ == '__main__':
    unittest.main()
