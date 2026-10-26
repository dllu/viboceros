"""Frozen observations distinguish joining from independent trimmed fits."""
import gzip
import hashlib
import json
from pathlib import Path
import unittest
from .generate_loft_refit_subdivision import request

HERE = Path(__file__).resolve().parent


class RefitSubdivisionReferences(unittest.TestCase):
    def test_frozen_recipe_archives_and_source_purity(self):
        encoded = (HERE / 'fixtures/loft_refit_subdivision.json').read_bytes()
        query = json.loads(encoded)
        native = json.loads((HERE / 'observations/loft_refit_subdivision.json').read_text())
        self.assertEqual(query, request())
        self.assertEqual(len(query['operations']), 45)
        self.assertEqual([op['id'] for op in query['operations']], [row['id'] for row in native['results']])
        self.assertEqual(hashlib.sha256(encoded).hexdigest(), native['provenance']['request_sha256'])
        for name, digest in native['provenance']['sources'].items():
            data = gzip.decompress((HERE / ('observations/source_snapshots/' + digest + '.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(data).hexdigest(), digest, name)
        for op, row in zip(query['operations'], native['results']):
            if op['op'] == 'loft':
                self.assertEqual(row['value']['before'], row['value']['after'])

    def test_joined_and_separately_trimmed_quarters_have_distinct_fits(self):
        data = json.loads((HERE / 'observations/loft_refit_subdivision.json').read_text())
        rows = {row['id']: row['value'] for row in data['results']}
        prefix = 'loft-refit-subdivision-quarter-'
        full = rows[prefix + 'full-0.001']['surfaces'][0]
        self.assertEqual(full['control_count'][1], 6)
        outer = full['control_points'][full['control_count'][0]]['point'][1]
        for side in ['left', 'right']:
            a = rows[prefix + side + '-unit-domain-0.001']['surfaces'][0]
            b = rows[prefix + side + '-source-domain-0.001']['surfaces'][0]
            self.assertEqual(a, b)
            self.assertEqual(a['control_count'][1], 4)
            controls = a['control_points'][::a['control_count'][0]]
            h = sum((x - y) ** 2 for x, y in zip(controls[1]['point'], controls[0]['point'])) ** .5
            self.assertGreater(abs(h - outer), 1e-8)


if __name__ == '__main__':
    unittest.main()
