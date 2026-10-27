"""Frozen fitting and derivative evidence across equivalent representations."""
import gzip
import hashlib
import json
from pathlib import Path
import unittest
from .generate_loft_refit_source_continuity import request

HERE = Path(__file__).resolve().parent


def difference(a, b):
    if isinstance(a, dict):
        if set(a) != set(b):
            raise AssertionError('different definition fields')
        return max([0.] + [difference(a[k], b[k]) for k in a])
    if isinstance(a, list):
        if len(a) != len(b):
            raise AssertionError('different definition lengths')
        return max([0.] + [difference(x, y) for x, y in zip(a, b)])
    if type(a) in (int, float) and type(b) in (int, float):
        return abs(a - b)
    if a != b:
        raise AssertionError('different definition values')
    return 0.


class SourceContinuityTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        family = 'loft_refit_source_continuity'
        cls.encoded = (HERE / ('fixtures/' + family + '.json')).read_bytes()
        cls.query = json.loads(cls.encoded)
        cls.native = json.loads((HERE / ('observations/' + family + '.json')).read_text())

    def test_recipe_archives_order_and_unchanged_native_inputs(self):
        self.assertEqual(self.query, request())
        self.assertEqual(len(self.query['operations']), 64)
        self.assertEqual([o['id'] for o in self.query['operations']], [r['id'] for r in self.native['results']])
        self.assertEqual(hashlib.sha256(self.encoded).hexdigest(), self.native['provenance']['request_sha256'])
        for name, digest in self.native['provenance']['sources'].items():
            content = gzip.decompress((HERE / ('observations/source_snapshots/' + digest + '.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(content).hexdigest(), digest, name)
        for row in self.native['results']:
            self.assertEqual(row['value']['before'], row['value']['after'])

    def test_native_fitting_is_invariant_under_degree_domain_and_simplification(self):
        rows = self.native['results']
        for i in range(0, 64, 4):
            base = rows[i]['value']
            for row in rows[i:i + 4]:
                self.assertLessEqual(difference(base['surfaces'], row['value']['surfaces']), 1e-9)
                sources = row['value']['refit_sources']
                for method in range(4):
                    self.assertLessEqual(difference(base['refit_sources'][method]['curves'],
                        sources[method]['curves']), 1e-9)
                for method in [0, 1]:
                    self.assertLessEqual(difference(sources[method]['curves'], sources[method + 2]['curves']), 1e-9)

    def test_c1_breaks_do_not_alone_establish_the_fitting_topology(self):
        for row in self.native['results']:
            sources = row['value']['refit_sources']
            for source in sources[:4]:
                info = source['source_structure']
                self.assertEqual(len(info['discontinuities']['C1_continuous']), 1)
                self.assertEqual(info['discontinuities']['G1_continuous'], [])
                self.assertEqual(info['discontinuities']['G2_continuous'], [])
                self.assertEqual(len(info['spans']), 2)
            self.assertEqual(sources[2]['source_types'], ['PolylineCurve'])
        counts = [r['value']['surfaces'][0]['control_count'][1] for r in self.native['results']]
        self.assertEqual(set(counts), {8, 12, 14, 16})


if __name__ == '__main__':
    unittest.main()
