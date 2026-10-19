"""Public InsertEditPoint source closure, complete captures and bounded recipes."""
import copy
import gzip
import hashlib
import json
import tempfile
from pathlib import Path
from unittest import TestCase
from .generate_insert_edit_point import request
from .insert_edit_point_probe import validate
from .insert_edit_point_limits_probe import validate as validate_limits
from .insert_edit_point_history_probe import validate as validate_history
from .structural_sdk import stage_helpers, validate_operations

HERE = Path(__file__).parent
COUNTS = {'insert_edit_point_command': 152, 'insert_edit_point_limits': 48,
          'insert_edit_point_projection': 16, 'insert_edit_point_history': 24, 'empty_undo_selection': 4}


class InsertEditPointTests(TestCase):
    def test_native_provenance_geometry_and_history_records(self):
        digest = lambda data: hashlib.sha256(data).hexdigest()
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
            for op, row in zip(q['operations'], r['results']):
                self.assertNotIn('error', row)
                value = row['value']
                if stem == 'empty_undo_selection':
                    self.assertTrue(value['steps'][0]['after_script']['selected'])
                    self.assertEqual(value['steps'][1]['after_script']['marker'], op['redo'])
                elif stem == 'insert_edit_point_history':
                    self.assertEqual([s['command'] for s in value['steps']], ['InsertEditPoint', 'Undo', 'Redo'])
                    for step in value['steps']:
                        self.assertEqual(step['after'], step['after_script'])
                    # This witness distinguishes equal-geometry replacement
                    # from preserving the old redo branch on cancellation.
                    self.assertEqual(value['steps'][-1]['after_script']['marker'], op['cancel'])
                else:
                    self.assertIn('after', value)
                    self.assertIn('undo', value)
                    self.assertIn('redo', value)
                    self.assertEqual(value['recipe']['id'], op['id'])
            if stem == 'insert_edit_point_command':
                generated = request()
                for a, b in zip(generated['operations'], q['operations']):
                    self.assertEqual(Path(a.pop('artifact_path')).name, Path(b.pop('artifact_path')).name)
                self.assertEqual(generated, q)

    def test_standalone_staging_and_recipe_rejection(self):
        q = request()
        with tempfile.TemporaryDirectory() as d:
            stage_helpers(q, HERE, Path(d))
            self.assertEqual({p.name for p in Path(d).iterdir()}, {
                'insert_edit_point_probe.py', 'insert_kink_probe.py', 'join_probe.py', 'merge_edges_probe.py'})
            for path in Path(d).iterdir():
                self.assertEqual(path.read_bytes(), (HERE / path.name).read_bytes())
        base = q['operations'][0]
        bad = [dict(stations=[True]), dict(stations=[float('nan')]), dict(stations=[2.]),
               dict(stations=['.3 _Delete']), dict(id='bad\n_Delete'), dict(family='surface'),
               dict(preselect=1), dict(midpoints=True), dict(extra=True)]
        for patch in bad:
            with self.subTest(patch=patch), self.assertRaises(ValueError):
                validate(dict(copy.deepcopy(base), **patch))
        limits = dict(base, op='insert_edit_point_limits', scale=1.)
        for patch in [dict(scale=True), dict(scale=0.), dict(scale=float('nan')), dict(scale=1e7), dict(family='circle', scale=2.)]:
            with self.subTest(patch=patch), self.assertRaises(ValueError):
                validate_limits(dict(limits, **patch))
        history = dict(op='insert_edit_point_history', id='history', station='empty', cancel=False, preselect=True, geometry_text=False)
        validate_history(history)
        for patch in [dict(station='new _Delete'), dict(cancel=1), dict(extra=True)]:
            with self.subTest(patch=patch), self.assertRaises(ValueError):
                validate_history(dict(history, **patch))
