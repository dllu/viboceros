"""Actual command evidence, distinct from the legacy grip-Delete SDK matrix."""
import copy
import gzip
import hashlib
import json
import tempfile
from collections import Counter
from pathlib import Path
from unittest import TestCase
from .generate_remove_control_point import request
from .remove_control_point_probe import validate
from .structural_sdk import stage_helpers

HERE = Path(__file__).parent


class RemoveControlPointCommands(TestCase):
    def test_bounded_actions_and_exact_staged_helpers(self):
        q = request()
        self.assertEqual(len(q['operations']), 133)
        self.assertEqual(len({o['id'] for o in q['operations']}), 133)
        with tempfile.TemporaryDirectory() as d:
            stage_helpers(q, HERE, Path(d))
            self.assertEqual({p.name for p in Path(d).iterdir()}, {
                'remove_control_point_probe.py', 'remove_knot_command_probe.py',
                'insert_kink_probe.py', 'join_probe.py', 'merge_edges_probe.py'})
            for p in Path(d).iterdir():
                self.assertEqual(p.read_bytes(), (HERE / p.name).read_bytes())
        base = q['operations'][0]
        for actions in (None, [True], ['Undo'], ['!'], ['Delete'], [dict(grip=-1)],
                        [dict(grip=True)], [dict(grip=[1, 2, 3])], [dict(point=[0., 0., float('nan')])],
                        [dict(point=[True, 0., 0.])], [dict(grip=1)] * 2):
            with self.subTest(actions=actions), self.assertRaises(ValueError):
                validate(dict(copy.deepcopy(base), actions=actions))

    def test_frozen_native_results_and_source_hashes(self):
        capture = json.loads((HERE / 'observations/remove_control_point_command.json').read_text())
        q = request()
        self.assertEqual([r['id'] for r in capture['results']], [o['id'] for o in q['operations']])
        outcomes = Counter()
        for row, op in zip(capture['results'], q['operations']):
            v = row['value']
            a, b = dict(v['recipe']), dict(op)
            self.assertEqual(Path(a.pop('artifact_path')).name, Path(b.pop('artifact_path')).name)
            self.assertEqual(a, b)
            events = [e for e in v['events'] if e['name'] == 'RemoveControlPoint']
            self.assertEqual(len(events), 1, row['id'])
            outcomes[events[0]['result']] += 1
            self.assertEqual(v['success'], events[0]['result'] == 'Success')
            self.assertTrue(v['macro'].startswith('_RemoveControlPoint '))
            self.assertLessEqual(len(v['picks']), 1)
            if op['family'] not in ('line', 'box'):
                self.assertNotIn('Unknown command:', v['history'], row['id'])
            self.assertFalse(v['after'][0]['selected'])
            for key in ('name', 'layer', 'groups', 'user_text'):
                self.assertEqual(v['after'][0][key], v['before'][0][key], (row['id'], key))
            p = Path(op['artifact_path'])
            self.assertEqual(hashlib.sha256(p.read_bytes()).hexdigest(), capture['provenance']['fixtures'][p.name])
        self.assertEqual(outcomes, dict(Success=132, Cancel=1))
        for name, digest in capture['provenance']['sources'].items():
            data = (HERE / name).read_bytes()
            if hashlib.sha256(data).hexdigest() != digest and name in ('rhino_worker.py', 'structural_sdk.py'):
                data = gzip.decompress((HERE / 'observations/source_snapshots' / (digest + '.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(data).hexdigest(), digest, name)
