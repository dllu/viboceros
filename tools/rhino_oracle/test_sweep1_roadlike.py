"""Roadlike command axes, documented defaults, and immutable SDK diagnostics."""
import gzip
import hashlib
import json
from pathlib import Path
from types import SimpleNamespace
import unittest
from .generate_sweep1_roadlike_default import request

HERE = Path(__file__).resolve().parent


class SweepRoadlikeTests(unittest.TestCase):
    def test_frozen_requests_and_distinct_native_producers(self):
        workers = {}
        for family, query in [
            ('sweep1_roadlike_explicit', request(explicit_axes=True)),
            ('sweep1_roadlike_automatic', request(command=False)),
            ('sweep1_roadlike_sdk_default', request(command=False)),
        ]:
            with self.subTest(family=family):
                fixture = HERE / ('fixtures/' + family + '.json')
                observed = json.loads((HERE / ('observations/' + family + '.json')).read_text())
                self.assertEqual(json.loads(fixture.read_text()), query)
                self.assertEqual(observed['engine'], 'rhino')
                self.assertEqual(observed['provenance']['request_sha256'],
                                 hashlib.sha256(fixture.read_bytes()).hexdigest())
                self.assertEqual([r['id'] for r in observed['results']],
                                 [op['id'] for op in query['operations']])
                for name, digest in observed['provenance']['sources'].items():
                    data = gzip.decompress((HERE / ('observations/source_snapshots/' + digest + '.gz')).read_bytes())
                    self.assertEqual(hashlib.sha256(data).hexdigest(), digest)
                    if name == 'rhino_worker.py':
                        workers[family] = data.decode()
                empty = [r['id'] for r in observed['results'] if not r['value']]
                expected_empty = [op['id'] for op in query['operations'] if op['rail']['type'] == 'arc']
                self.assertEqual(empty, expected_empty if family.endswith('sdk_default') else [])
        self.assertNotEqual(workers['sweep1_roadlike_sdk_default'], workers['sweep1_roadlike_automatic'])
        self.assertNotIn('rail.TryGetPlane', workers['sweep1_roadlike_sdk_default'])
        self.assertIn('rail.TryGetPlane', workers['sweep1_roadlike_automatic'])

    def test_axis_getter_receives_explicit_points_or_cplane_acceptance(self):
        from .sweep1_probe import macro
        explicit = macro({'roadlike_axis': [0, 1, 0]}, ['rail', 'profile'])
        self.assertIn(' _Style=_Roadlike 0,0,0 0,1,0 _Simplify=', explicit)
        automatic = macro({'roadlike': True}, ['rail', 'profile'])
        self.assertIn(' _Style=_Roadlike _Enter _Simplify=', automatic)
        closed = macro({'roadlike_axis': [0, 1, 0], 'end_point': [0, 0, 5]}, ['rail', 'profile'], True)
        self.assertIn(' _Point 0,0,5 _Enter _Style=_Roadlike 0,0,0 0,1,0', closed)

    def test_sdk_default_uses_public_plane_query_without_overriding_explicit_axis(self):
        from .sweep1_probe import roadlike_axis
        axis = lambda op, rail, tol: roadlike_axis(op, rail, tol, list)
        class Rail:
            def __init__(self, planar):
                self.planar = planar
                self.queries = []
            def TryGetPlane(self, tolerance):
                self.queries.append(tolerance)
                return self.planar, SimpleNamespace(ZAxis=(0, -1, 0))
        tolerance = dict(absolute=1e-7)
        planar, spatial = Rail(True), Rail(False)
        self.assertEqual(axis({'roadlike': True}, planar, tolerance), [0, -1, 0])
        self.assertEqual(axis({'roadlike': True}, spatial, tolerance), [0, 0, 1])
        self.assertEqual(planar.queries, [1e-7])
        planar.queries.clear()
        self.assertEqual(axis({'roadlike': True, 'roadlike_axis': [1, 0, 0]}, planar, tolerance), [1, 0, 0])
        self.assertIsNone(axis({}, planar, tolerance))
        self.assertEqual(planar.queries, [])

    def test_validation_binds_complete_images_and_current_sources(self):
        from .qualification_sources import with_shared_sources
        root = HERE.parent.parent
        record = json.loads((root / 'docs/sweep1/roadlike-validation.json').read_text())
        self.assertTrue(record['documented_default_axis_implemented'])
        self.assertFalse(record['complete_rhino_sweep_compatibility'])
        for key in ['portable_command_images', 'native_smlib_command_images']:
            rows = record[key]
            self.assertEqual(len(rows), 11)
            self.assertEqual(len({r['id'] for r in rows}), 11)
            self.assertTrue(all(0 <= r['complete_image_bound'] <= r['comparison_epsilon'] for r in rows))
        for check in record['checks'].values():
            self.assertEqual(check['exit_code'], 0)
        self.assertEqual(set(record['sha256']), set(with_shared_sources(list(record['sha256']))))
        for path, digest in record['sha256'].items():
            self.assertEqual(hashlib.sha256((root/path).read_bytes()).hexdigest(), digest, path)


if __name__ == '__main__':
    unittest.main()
