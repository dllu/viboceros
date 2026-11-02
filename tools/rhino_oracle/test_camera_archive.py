"""Native camera archives retain the cached frame independently of raw vectors."""
import copy
import gzip
import hashlib
import json
import unittest
from .camera_archive_probe import validate
from .generate_camera_archive import FAMILY, HERE, request


class CameraArchiveTests(unittest.TestCase):
    def test_native_archives_and_frozen_producers(self):
        fixture = HERE / ('fixtures/' + FAMILY + '.json')
        q = json.loads(fixture.read_text())
        n = json.loads((HERE / ('observations/' + FAMILY + '.json')).read_text())
        self.assertEqual(q, request())
        self.assertEqual(len(q['operations']), 18)
        self.assertEqual(len(n['results']), 18)
        self.assertEqual(n['engine'], 'rhino')
        self.assertEqual(n['provenance']['request_sha256'], hashlib.sha256(fixture.read_bytes()).hexdigest())
        for name, sha in n['provenance']['sources'].items():
            data = gzip.decompress((HERE / ('observations/source_snapshots/' + sha + '.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(data).hexdigest(), sha, name)
        distinct = []
        for op, row in zip(q['operations'], n['results']):
            with self.subTest(case=op['id']):
                self.assertEqual(op['id'], row['id'])
                self.assertNotIn('error', row)
                value = row['value']
                self.assertEqual(value['file'], 'fixtures/camera_archive/' + op['id'] + '.3dm')
                self.assertEqual(hashlib.sha256((HERE / value['file']).read_bytes()).hexdigest(), value['file_sha256'])
                for state in ('archive', 'named_restore'):
                    for key in ('direction', 'up', 'location', 'frame'):
                        self.assertEqual(value[state][key], value['source'][key])
                if not op['rotate']:
                    self.assertEqual(value['source']['direction'], op['direction'])
                    self.assertEqual(value['source']['up'], op['up'])
                a = value['source']['frame'][2]
                b = value['installed']['frame'][2]
                if sum((x-y)**2 for x, y in zip(a, b)) > 1e-20:
                    self.assertEqual(op['projection'], 'TwoPointPerspective')
                    self.assertTrue(op['rotate'])
                    distinct.append(op['id'])
        self.assertEqual(distinct, ['TwoPointPerspective-skew-1', 'TwoPointPerspective-tilted-1'])

    def test_supplemental_native_projection_samples(self):
        from .generate_camera_archive_projection import FAMILY as family, request as query
        fixture = HERE / ('fixtures/' + family + '.json')
        q = json.loads(fixture.read_text())
        n = json.loads((HERE / ('observations/' + family + '.json')).read_text())
        self.assertEqual(q, query())
        self.assertEqual(len(n['results']), 2)
        self.assertEqual(n['provenance']['request_sha256'], hashlib.sha256(fixture.read_bytes()).hexdigest())
        for name, sha in n['provenance']['sources'].items():
            data = gzip.decompress((HERE / ('observations/source_snapshots/' + sha + '.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(data).hexdigest(), sha, name)
        for op, row in zip(q['operations'], n['results']):
            self.assertEqual(row['id'], op['id'])
            value = row['value']
            self.assertEqual(hashlib.sha256((HERE / value['file']).read_bytes()).hexdigest(), value['file_sha256'])
            for state in ('source', 'named_restore'):
                view = value[state]
                self.assertEqual(len(view['points']), 6)
                for point in view['points'][3:]:
                    depth = -sum((x-c)*z for x, c, z in zip(point['world'], view['location'], view['frame'][2]))
                    self.assertGreater(depth, 9.)

    def test_rejects_unsafe_or_ambiguous_recipes(self):
        base = request()['operations'][0]
        for changes in ({'id': '../file'}, {'projection': 'bad'}, {'rotate': 1},
                        {'up': [0, 0, 0]}, {'direction': [True, 1, 0]},
                        {'direction': [float('nan'), 0, 1]}, {'up': [0, 1e51, 0]},
                        {'extra': 'ignored'}, {'camera_samples': 1}):
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                validate(dict(copy.deepcopy(base), **changes))


if __name__ == '__main__':
    unittest.main()
