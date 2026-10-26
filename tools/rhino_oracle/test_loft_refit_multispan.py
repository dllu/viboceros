"""Frozen multispan initial proposals and adaptive-fitting counterexamples."""
import gzip
import hashlib
import json
from pathlib import Path
import unittest
from .generate_loft_refit_multispan import request as profiles
from .generate_loft_refit_multispan_commands import request as commands

HERE = Path(__file__).resolve().parent


class MultispanRefitTests(unittest.TestCase):
    def test_recipes_archives_coverage_and_native_sources(self):
        for family, recipe, count in [('loft_refit_multispan', profiles, 66),
                                     ('loft_refit_multispan_commands', commands, 6)]:
            encoded = (HERE / ('fixtures/' + family + '.json')).read_bytes()
            query = json.loads(encoded)
            native = json.loads((HERE / ('observations/' + family + '.json')).read_text())
            self.assertEqual(query, recipe())
            self.assertEqual(len(query['operations']), count)
            self.assertEqual([o['id'] for o in query['operations']],
                             [r['id'] for r in native['results']])
            self.assertEqual(hashlib.sha256(encoded).hexdigest(),
                             native['provenance']['request_sha256'])
            for name, digest in native['provenance']['sources'].items():
                content = gzip.decompress((HERE / ('observations/source_snapshots/'
                                                   + digest + '.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(content).hexdigest(), digest, name)
            for row in native['results']:
                if family == 'loft_refit_multispan':
                    self.assertEqual(row['value']['before'], row['value']['after'])
                else:
                    value = row['value']
                    self.assertTrue(value['succeeded'])
                    self.assertTrue(all(value['originals_present']))
                    self.assertTrue(all(value['originals_selected']))
                    self.assertEqual(len(value['outputs']), 1)
                    self.assertTrue(value['outputs'][0]['valid'])

    def test_curvature_changes_can_fit_once_but_tighter_bends_need_subdivision(self):
        native = json.loads((HERE / 'observations/loft_refit_multispan.json').read_text())
        rows = {r['id']: r['value']['surfaces'][0] for r in native['results']}
        self.assertEqual(sum(r['control_count'][1] == 4 for r in rows.values()), 30)
        for variant in ['plain', 'domain', 'frame']:
            prefix = 'loft-refit-multispan-curvature-jump-' + variant + '-'
            self.assertEqual(rows[prefix + '0.001']['control_count'][1], 4)
            self.assertGreater(rows[prefix + '1e-06']['control_count'][1], 4)
            prefix = 'loft-refit-multispan-angle-0.25-' + variant + '-'
            self.assertGreater(rows[prefix + '0.001']['control_count'][1], 4)


if __name__ == '__main__':
    unittest.main()
