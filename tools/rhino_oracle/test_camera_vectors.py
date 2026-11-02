"""Immutable camera metadata, public frames and native draft View controls."""
import copy
import gzip
import hashlib
import json
import math
import unittest
from .camera_vectors_probe import validate
from .capture_family import HERE
from .generate_camera_vectors import FAMILY, request


class CameraVectorTests(unittest.TestCase):
    def test_frozen_native_camera_vectors_and_frames(self):
        path = HERE / ('fixtures/' + FAMILY + '.json')
        q = json.loads(path.read_text())
        n = json.loads((HERE / ('observations/' + FAMILY + '.json')).read_text())
        self.assertEqual(q, request())
        self.assertEqual(len(q['operations']), 35)
        self.assertEqual(n['provenance']['request_sha256'], hashlib.sha256(path.read_bytes()).hexdigest())
        for name, sha in n['provenance']['sources'].items():
            raw = gzip.decompress((HERE / ('observations/source_snapshots/' + sha + '.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(raw).hexdigest(), sha, name)
        self.assertEqual(len(n['results']), len(q['operations']))
        for op, row in zip(q['operations'], n['results']):
            with self.subTest(case=op['id']):
                self.assertEqual(op['id'], row['id'])
                self.assertNotIn('error', row)
                value = row['value']
                if op['op'] == 'draft_pull_direction':
                    self.assertEqual(value['camera'], op['view'])
                    self.assertEqual(value['selected'], [-x for x in op['view']])
                    self.assertEqual(value['up'], value['selected'])
                    self.assertEqual(value['input']['errors'], [])
                    self.assertEqual(value['repeated']['errors'], [])
                    continue
                for state in ('before', 'roundtrip'):
                    for key in ('direction', 'up'):
                        self.assertEqual(value[state][key], op[key])
                for state in ('before', 'roundtrip', 'after'):
                    frame = value[state]['frame']
                    for i in range(3):
                        self.assertAlmostEqual(sum(x*x for x in frame[i]), 1., places=12)
                        for j in range(i):
                            self.assertAlmostEqual(sum(x*y for x, y in zip(frame[i], frame[j])), 0., places=12)
                after = value['after']
                if op['action'] in ('None', 'Zoom', 'MousePan', 'MouseZoom'):
                    self.assertEqual(after['direction'], op['direction'])
                    self.assertEqual(after['up'], op['up'])
                elif op['action'] in ('WorldTop', 'Plan'):
                    self.assertEqual(after['direction'], [0, 0, -1])
                    self.assertEqual(after['up'], [0, 1, 0])
                else:
                    self.assertAlmostEqual(math.sqrt(sum(x*x for x in after['direction'])), 1., places=12)
                    if op['projection'] == 'TwoPointPerspective':
                        self.assertEqual(after['up'], op['up'])
                    else:
                        self.assertAlmostEqual(math.sqrt(sum(x*x for x in after['up'])), 1., places=12)

    def test_rejects_invalid_or_unbounded_camera_input(self):
        base = request()['operations'][0]
        for changes in ({'projection': 'bad'}, {'action': 'bad'}, {'id': '../file'},
                        {'direction': [0, 0, 0]}, {'up': [0, float('inf'), 0]},
                        {'direction': [True, 1, 0]}, {'up': [0, 1e51, 0]},
                        {'mouse_drag': [12, -7]}, {'action': 'PhysicalOrbit'},
                        {'action': 'PhysicalOrbit', 'mouse_drag': [51, 0]},
                        {'action': 'PhysicalOrbit', 'mouse_drag': [True, 0]}):
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                validate(dict(copy.deepcopy(base), **changes))


if __name__ == '__main__':
    unittest.main()
