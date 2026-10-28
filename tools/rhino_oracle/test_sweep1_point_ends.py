"""Frozen point-end captures and actual scripted selection/option transitions."""
import gzip
import hashlib
import json
from pathlib import Path
import unittest
from .generate_sweep1_point_ends import request
from .generate_sweep1_point_ends_closed import request as closed_request
from .generate_sweep1_global_basis import request as basis_request
from .generate_sweep1_global_dense import request as dense_request
from .generate_sweep1_global_retained import request as retained_request

HERE = Path(__file__).resolve().parent


class SweepPointEndsTests(unittest.TestCase):
    def test_requests_and_native_producers_are_frozen(self):
        recipes = [
            ('sweep1_point_ends', request()),
            ('sweep1_point_ends_commands', request(command=True)),
            ('sweep1_point_ends_retained', request(command=True, refit=False)),
            ('sweep1_point_ends_closed', closed_request()),
            ('sweep1_global_basis', basis_request()),
            ('sweep1_global_basis_loose', basis_request(absolute=1e-3)),
            ('sweep1_global_dense', dense_request()),
            ('sweep1_global_retained', retained_request()),
        ]
        for family, expected in recipes:
            with self.subTest(family=family):
                fixture = HERE / ('fixtures/' + family + '.json')
                query = json.loads(fixture.read_text())
                capture = json.loads((HERE / ('observations/' + family + '.json')).read_text())
                self.assertEqual(query, expected)
                self.assertEqual(capture['engine'], 'rhino')
                self.assertEqual([row['id'] for row in capture['results']],
                                 [op['id'] for op in query['operations']])
                self.assertEqual(hashlib.sha256(fixture.read_bytes()).hexdigest(),
                                 capture['provenance']['request_sha256'])
                for digest in capture['provenance']['sources'].values():
                    data = gzip.decompress((HERE / ('observations/source_snapshots/' + digest + '.gz')).read_bytes())
                    self.assertEqual(hashlib.sha256(data).hexdigest(), digest)
                for row in capture['results']:
                    self.assertEqual(len(row['value']), 1)
                    self.assertTrue(all(c['weight'] > 0 for c in row['value'][0]['definition']['control_points']))

    def test_validation_binds_sources_and_complete_image_checks(self):
        from .qualification_sources import with_shared_sources
        root = HERE.parent.parent
        record = json.loads((root / 'docs/sweep1/point-ends-validation.json').read_text())
        self.assertTrue(record['point_ends_implemented'])
        self.assertFalse(record['complete_rhino_sweep_compatibility'])
        self.assertFalse(record['global_multi_profile_policy_implemented'])
        self.assertEqual(record['construction_tolerance'], 1e-7)
        self.assertEqual(record['comparison_epsilon'], 1e-7)
        for key in ['kernel_complete_images', 'portable_command_complete_images',
                    'native_smlib_command_complete_images']:
            rows = record[key]
            self.assertEqual(len(rows), 12)
            self.assertEqual(len({(r['family'], r['id']) for r in rows}), 12)
            self.assertTrue(all(0 <= r['complete_image_bound'] <= 1e-7 for r in rows))
        self.assertEqual(set(record['sha256']), set(with_shared_sources(list(record['sha256']))))
        for path, digest in record['sha256'].items():
            self.assertEqual(hashlib.sha256((root/path).read_bytes()).hexdigest(), digest, path)
        for check in record['checks'].values():
            self.assertEqual(check['exit_code'], 0)

    def test_macro_advances_through_point_and_closed_seam_prompts(self):
        from .sweep1_probe import macro
        plain = macro({}, ['rail', 'profile'])
        self.assertEqual(plain, "_-Sweep1 '_-SelID rail '_-SelID profile _Enter _Style=_Freeform _Simplify=_None _Closed=_No _ShapeBlending=_Local _RefitRail=_No _Enter")
        ended = macro({'start_point': [0, 0, 0], 'end_point': [0, 0, 5], 'refit_rail': True}, ['rail', 'profile'])
        self.assertIn(" _Point 0,0,0 '_-SelID profile _Point 0,0,5 _Style=", ended)
        closed = macro({'end_point': [0, 0, 5]}, ['rail', 'profile'], True)
        self.assertIn(" _Point 0,0,5 _Enter _Style=", closed)
        self.assertIn("_RefitRail=_Yes", ended)


if __name__ == '__main__':
    unittest.main()
