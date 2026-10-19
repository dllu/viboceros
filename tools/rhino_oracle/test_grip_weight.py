"""Freeze native Weight evidence and reject incomplete recipes/comparisons."""
import copy
import gzip
import hashlib
import json
import tempfile
from pathlib import Path
from unittest import TestCase
from .generate_grip_weight import request
from .grip_weight_probe import validate
from .weight_lifecycle_probe import validate as validate_lifecycle
from .structural_sdk import stage_helpers, validate_operations

HERE = Path(__file__).parent
COUNTS = {'grip_weight': 237, 'grip_weight_selection': 5,
          'grip_weight_representations': 20, 'grip_weight_lifecycle': 14}


class GripWeightTests(TestCase):
    def test_native_capture_provenance_and_complete_states(self):
        digest = lambda data: hashlib.sha256(data).hexdigest()
        boundary_differences = 0
        for stem, count in COUNTS.items():
            path = HERE / 'fixtures' / (stem + '.json')
            q = json.loads(path.read_text())
            r = json.loads((HERE / 'observations' / (stem + '.json')).read_text())
            self.assertEqual(len(r['results']), count)
            self.assertEqual([o['id'] for o in q['operations']], [o['id'] for o in r['results']])
            self.assertEqual(digest(path.read_bytes()), r['provenance']['request_sha256'])
            validate_operations(q)
            for name, expected in r['provenance']['sources'].items():
                data = (HERE / name).read_bytes()
                if digest(data) != expected:
                    data = gzip.decompress((HERE / 'observations/source_snapshots' / (expected + '.gz')).read_bytes())
                self.assertEqual(digest(data), expected, name)
            for name, expected in r['provenance'].get('fixtures', {}).items():
                self.assertEqual(digest((HERE / 'fixtures' / stem / name).read_bytes()), expected)
            if 'diagnostics_sha256' in r['provenance']:
                self.assertEqual(digest((HERE / 'observations' / (stem + '_input.json')).read_bytes()), r['provenance']['diagnostics_sha256'])
            for row in r['results']:
                self.assertNotIn('error', row)
                for step in row['value']['steps']:
                    self.assertIn('before', step)
                    self.assertIn('after_script', step)
                    boundary_differences += step['after'] != step['after_script']
                    if stem != 'grip_weight_lifecycle':
                        for obj in step['after_script']['objects']:
                            for grip in obj['grips']:
                                self.assertIn('weight', grip)
            if stem == 'grip_weight':
                generated = request()
                for a, b in zip(generated['operations'], q['operations']):
                    self.assertEqual(Path(a.pop('artifact_path')).name, Path(b.pop('artifact_path')).name)
                self.assertEqual(generated, q)
                self.assertEqual(sum(len(o['value']['steps']) for o in r['results']), 721)
        self.assertGreater(boundary_differences, 0)

    def test_standalone_staging_and_closed_recipes(self):
        q = request()
        with tempfile.TemporaryDirectory() as d:
            stage_helpers(q, HERE, Path(d))
            self.assertEqual({p.name for p in Path(d).iterdir()}, {
                'grip_weight_probe.py', 'weight_dialog.py', 'delete_grips_probe.py',
                'grip_navigation_probe.py', 'grip_transform_probe.py', 'join_probe.py', 'merge_edges_probe.py'})
            for path in Path(d).iterdir():
                self.assertEqual(path.read_bytes(), (HERE / path.name).read_bytes())
        base = q['operations'][0]
        bad = [dict(id='bad\n_Delete'), dict(peer=1), dict(off=1), dict(sources=[]),
               dict(extra=True), dict(inputs=[]), dict(inputs=[True]), dict(inputs=[float('nan')]),
               dict(inputs=[float('inf')]), dict(inputs=['2 _Delete']), dict(inputs=[1000001]),
               dict(dash=False, inputs=[2, 3]), dict(pick=1), dict(repeat=1)]
        bad += [dict(sources=[dict(base['sources'][0], **s)]) for s in
                [dict(kind='unknown'), dict(mode='surface'), dict(selected='all _Delete'),
                 dict(parent=1), dict(hidden='index'), dict(extra=True),
                 dict(kind='mesh', mode='edit'), dict(kind='curve', selected='row_u')]]
        for patch in bad:
            with self.subTest(patch=patch), self.assertRaises(ValueError):
                validate(dict(copy.deepcopy(base), **patch))
        q = json.loads((HERE / 'fixtures/grip_weight_lifecycle.json').read_text())
        for op in q['operations']:
            validate_lifecycle(op)
        for patch in [dict(preset=1), dict(kind='mesh'), dict(seed='all'), dict(extra=True)]:
            with self.subTest(patch=patch), self.assertRaises(ValueError):
                validate_lifecycle(dict(q['operations'][0], **patch))

    def test_complete_comparison_detects_weight_and_history_regressions(self):
        from .client import _compare_value
        r = json.loads((HERE / 'observations/grip_weight.json').read_text())
        value = next(row['value'] for row in r['results'] if row['id'] == 'weighted_middle')
        paths = [('steps', 0, 'result'), ('steps', 0, 'getter', 'default'),
                 ('steps', 0, 'after', 'marker'), ('steps', 0, 'after_script', 'marker')]
        paths += [('steps', 0, 'after', 'objects', 1, k) for k in
                  ['same_id', 'selected', 'visible', 'grouped', 'grips_on', 'kind', 'name', 'color']]
        paths += [('steps', 0, 'after', 'objects', 1, 'grips', 0, k) for k in
                  ['selected', 'hidden', 'selectable', 'point', 'index', 'weight']]
        paths += [('steps', 0, 'after', 'objects', 1, 'curve', k) for k in
                  ['degree', 'domain', 'knots', 'control_points']]
        for path in paths:
            changed = copy.deepcopy(value)
            node = changed
            for key in path[:-1]:
                node = node[key]
            original = node[path[-1]]
            node[path[-1]] = not original if isinstance(original, bool) else None
            differences = []
            _compare_value('state', value, changed, 1e-8, 1e-12, differences)
            self.assertTrue(differences, path)
