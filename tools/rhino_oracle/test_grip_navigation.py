"""Closed recipes and complete directional selection command witnesses."""
import copy
import gzip
import hashlib
import json
import tempfile
from collections import Counter
from pathlib import Path
from unittest import TestCase
from .generate_grip_navigation import request, ROOT
from .grip_navigation_probe import COMMANDS, seed_indices, validate
from .structural_sdk import stage_helpers

HERE = Path(__file__).parent


class GripNavigationTests(TestCase):
    def test_complete_native_corpus_and_exact_provenance(self):
        q = request()
        path = HERE / 'fixtures/grip_navigation.json'
        recorded = json.loads(path.read_text())
        for a, b in zip(q['operations'], recorded['operations']):
            self.assertEqual(Path(a.pop('artifact_path')).name, Path(b.pop('artifact_path')).name)
        self.assertEqual(q, recorded)
        r = json.loads((HERE / 'observations/grip_navigation.json').read_text())
        self.assertEqual(len(r['results']), 25)
        self.assertEqual([o['id'] for o in q['operations']], [o['id'] for o in r['results']])
        self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(), r['provenance']['request_sha256'])
        results = Counter()
        for row in r['results']:
            v = row['value']
            self.assertEqual(len(v['steps']), 66)
            self.assertTrue(v['redo_retained'], row['id'])
            for step in v['steps']:
                self.assertIn(step['command'], COMMANDS)
                self.assertEqual(step['macro'], '_' + step['command'])
                self.assertEqual(step['before']['peer_selected'], step['after']['peer_selected'])
                for a, b in zip(step['before']['sources'], step['after']['sources']):
                    self.assertEqual(a['selected'], b['selected'])
                    self.assertEqual([(g['index'], g['point']) for g in a['grips']],
                                     [(g['index'], g['point']) for g in b['grips']])
                results[step['result']] += 1
        self.assertEqual(results, dict(Success=1295, Nothing=355))
        for name, digest in r['provenance']['sources'].items():
            data = (HERE / name).read_bytes()
            if hashlib.sha256(data).hexdigest() != digest and name in ('rhino_worker.py', 'structural_sdk.py'):
                data = gzip.decompress((HERE / 'observations/source_snapshots' / (digest + '.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(data).hexdigest(), digest, name)
        for name, digest in r['provenance']['fixtures'].items():
            self.assertEqual(hashlib.sha256((HERE / 'fixtures/grip_navigation' / name).read_bytes()).hexdigest(), digest)

    def test_standalone_staging_and_malformed_commands(self):
        q = request()
        with tempfile.TemporaryDirectory() as d:
            stage_helpers(q, HERE, Path(d))
            self.assertEqual({p.name for p in Path(d).iterdir()}, {
                'grip_navigation_probe.py', 'grip_transform_probe.py', 'join_probe.py', 'merge_edges_probe.py'})
            for p in Path(d).iterdir(): self.assertEqual(p.read_bytes(), (HERE / p.name).read_bytes())
        base = q['operations'][0]
        for op in (dict(base, sources=[True]), dict(base, extra=True), dict(base, actions=[]),
                   dict(base, actions=[dict(command='NextU _Delete')]),
                   dict(base, actions=[dict(command='SelUV')]),
                   dict(base, actions=[dict(command='NextU', seed='none')])):
            with self.subTest(op=op), self.assertRaises(ValueError): validate(copy.deepcopy(op))

    def test_periodic_and_clamped_closures_have_distinct_boundary_decisions(self):
        r = json.loads((HERE / 'observations/grip_navigation.json').read_text())
        rows = {row['id']: row['value'] for row in r['results']}
        pick = lambda step: [g['index'] for g in step['after']['sources'][0]['grips'] if g['selected']]
        self.assertEqual(pick(rows['closed_control']['steps'][1]), [])
        self.assertEqual(pick(rows['periodic_control']['steps'][1]), [3])
        self.assertEqual(pick(rows['curve_edit']['steps'][0]), [])
        self.assertEqual(pick(rows['mesh_control']['steps'][0]), [0])
        self.assertEqual(seed_indices('ends', 1), [0])
