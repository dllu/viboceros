"""Freeze public Delete observations, source closure and complete comparisons."""
import copy
import gzip
import hashlib
import json
import tempfile
from collections import Counter
from pathlib import Path
from unittest import TestCase
from .generate_delete_grips import request
from .delete_grips_probe import validate
from .grip_delete_sequence_probe import validate as validate_sequence
from .structural_sdk import stage_helpers

HERE = Path(__file__).parent


class DeleteGripsTests(TestCase):
    def test_capture_provenance_and_complete_states(self):
        digest = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
        for stem, count in [('delete_grips', 397), ('delete_grips_sequences', 109), ('delete_grips_sequences_extra', 6)]:
            path = HERE / 'fixtures' / (stem + '.json')
            q = json.loads(path.read_text())
            r = json.loads((HERE / 'observations' / (stem + '.json')).read_text())
            self.assertEqual(len(r['results']), count)
            self.assertEqual([o['id'] for o in q['operations']], [o['id'] for o in r['results']])
            self.assertEqual(digest(path), r['provenance']['request_sha256'])
            for name, expected in r['provenance']['sources'].items():
                data = (HERE / name).read_bytes()
                if hashlib.sha256(data).hexdigest() != expected and name in ('rhino_worker.py', 'structural_sdk.py'):
                    data = gzip.decompress((HERE / 'observations/source_snapshots' / (expected + '.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(data).hexdigest(), expected, name)
            for row in r['results']:
                self.assertNotIn('error', row)
                for step in row['value']['steps']:
                    self.assertEqual(step['after'], step['after_script'], row['id'])
            if stem == 'delete_grips':
                generated = request()
                for a, b in zip(generated['operations'], q['operations']):
                    self.assertEqual(Path(a.pop('artifact_path')).name, Path(b.pop('artifact_path')).name)
                self.assertEqual(generated, q)
                for name, expected in r['provenance']['fixtures'].items():
                    self.assertEqual(digest(HERE / 'fixtures/delete_grips' / name), expected)
                self.assertEqual(digest(HERE / 'observations/delete_grips_history.json'), r['provenance']['diagnostics_sha256'])
                steps = [s for row in r['results'] for s in row['value']['steps']]
                self.assertEqual(len(steps), 1193)
                self.assertEqual(set(s['command'] for s in steps), {'Delete', 'PointsOff', 'Undo', 'Redo'})
                self.assertEqual(Counter(s['result'] for s in steps), dict(Success=1048, Nothing=145))

    def test_standalone_staging_and_closed_recipes(self):
        q = request()
        with tempfile.TemporaryDirectory() as d:
            stage_helpers(q, HERE, Path(d))
            self.assertEqual({p.name for p in Path(d).iterdir()}, {
                'delete_grips_probe.py', 'grip_navigation_probe.py', 'grip_transform_probe.py',
                'join_probe.py', 'merge_edges_probe.py'})
            for p in Path(d).iterdir():
                self.assertEqual(p.read_bytes(), (HERE / p.name).read_bytes())
        base = q['operations'][0]
        source = base['sources'][0]
        bad = [dict(id='bad\n_Delete'), dict(peer=1), dict(off=1), dict(sources=[]), dict(extra=True)]
        bad += [dict(sources=[dict(source, **s)]) for s in [dict(kind='unknown'), dict(mode='surface'),
                dict(selected='all _Delete'), dict(parent=1), dict(hidden='index'), dict(extra=True),
                dict(kind='mesh', mode='edit'), dict(kind='curve', selected='row_u')]]
        for patch in bad:
            with self.subTest(patch=patch), self.assertRaises(ValueError):
                validate(dict(copy.deepcopy(base), **patch))
        for op in json.loads((HERE / 'fixtures/delete_grips_sequences.json').read_text())['operations']:
            validate_sequence(op)
        sequence = dict(op='grip_delete_sequence', id='bounded', kind='circle', selections=[[0]])
        for patch in [dict(selections=[[True]]), dict(selections=[[0, 0]]), dict(selections=[[16]]),
                      dict(kind='curve'), dict(degree=3), dict(selections=[]), dict(extra=True)]:
            with self.subTest(patch=patch), self.assertRaises(ValueError):
                validate_sequence(dict(sequence, **patch))

    def test_comparison_rejects_geometry_history_and_visibility_changes(self):
        from .client import _compare_value
        r = json.loads((HERE / 'observations/delete_grips.json').read_text())
        value = next(row['value'] for row in r['results'] if row['id'] == 'weighted_middle')
        paths = [('steps', 0, 'result'), ('steps', 0, 'after', 'marker')]
        paths += [('steps', 0, 'after', 'objects', 1, k) for k in
                  ['same_id', 'selected', 'visible', 'grouped', 'grips_on', 'kind', 'name', 'color']]
        paths += [('steps', 0, 'after', 'objects', 1, 'grips', 0, k) for k in
                  ['selected', 'hidden', 'selectable', 'point', 'index']]
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
