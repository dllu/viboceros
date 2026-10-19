"""Retain every public RemoveKnot recipe, source file and actual command outcome."""
import copy
import hashlib
import gzip
import json
import tempfile
from collections import Counter
from pathlib import Path
from unittest import TestCase
from .generate_remove_knot_command import request
from .remove_knot_command_probe import validate
from .structural_sdk import stage_helpers

HERE = Path(__file__).parent

class RemoveKnotCommands(TestCase):
    def test_bounded_request_and_staging(self):
        q = request()
        self.assertEqual(len(q['operations']), 149)
        self.assertEqual(len({o['id'] for o in q['operations']}), 149)
        with tempfile.TemporaryDirectory() as d:
            stage_helpers(q, HERE, Path(d))
            self.assertEqual({p.name for p in Path(d).iterdir()}, {
                'remove_knot_command_probe.py', 'insert_kink_probe.py', 'join_probe.py', 'merge_edges_probe.py'})
            for p in Path(d).iterdir():
                self.assertEqual(p.read_bytes(), (HERE / p.name).read_bytes())

    def test_invalid_actions_and_sources(self):
        base = request()['operations'][0]
        for actions in (None, [True], ['!'], ['Automatic'], ['Undo _Delete'],
                        [dict(point=[])], [dict(point=[.1,.2,.3])],
                        [dict(point=float('nan'))], [dict(point=True)],
                        [dict(parameter=float('inf'))], [dict(parameter=True)],
                        [dict(parameter=1, point=.2)], ['Undo'] * 33):
            op = copy.deepcopy(base)
            op['actions'] = actions
            with self.subTest(actions=actions), self.assertRaises(ValueError):
                validate(op)
        for change in (dict(seed=1), dict(deform=True, seed=False), dict(periodic_surface_axis='W')):
            with self.subTest(change=change), self.assertRaises(ValueError):
                validate(dict(base, **change))

    def test_frozen_native_results_and_provenance(self):
        capture = json.loads((HERE / 'observations/remove_knot_command.json').read_text())
        q = request()
        self.assertEqual([r['id'] for r in capture['results']], [o['id'] for o in q['operations']])
        outcomes = Counter()
        for row, op in zip(capture['results'], q['operations']):
            v = row['value']
            actual, expected = dict(v['recipe']), dict(op)
            self.assertEqual(Path(actual.pop('artifact_path')).name, Path(expected.pop('artifact_path')).name)
            self.assertEqual(actual, expected)
            events = [e for e in v['events'] if e['name'] == 'RemoveKnot']
            self.assertEqual(len(events), 1)
            outcomes[events[0]['result']] += 1
            self.assertEqual(v['success'], events[0]['result'] == 'Success')
            self.assertTrue(v['macro'].startswith('_RemoveKnot '))
            self.assertEqual(len(v['picks']), sum(isinstance(a, dict) and 'point' in a for a in op['actions']))
            original = next(o for o in v['after'] if o['id'] == v['source_id'])
            self.assertFalse(original['selected'])
            for key in ('name', 'layer', 'groups', 'user_text'):
                self.assertEqual(original[key], v['before'][0][key], (row['id'], key))
            path = Path(op['artifact_path'])
            self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(), capture['provenance']['fixtures'][path.name])
        self.assertEqual(outcomes, dict(Success=136, Cancel=12, Failure=1))
        for name, digest in capture['provenance']['sources'].items():
            data=(HERE/name).read_bytes()
            if hashlib.sha256(data).hexdigest()!=digest and name in('rhino_worker.py','structural_sdk.py'):
                data=gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(data).hexdigest(), digest, name)

    def test_followup_actions_and_frozen_sources(self):
        from .generate_remove_knot_followup import request as followup
        q = followup()
        capture = json.loads((HERE / 'observations/remove_knot_followup.json').read_text())
        self.assertEqual(len(q['operations']), 6)
        self.assertEqual([o['id'] for o in q['operations']], [r['id'] for r in capture['results']])
        for op, row in zip(q['operations'], capture['results']):
            v = row['value']
            actual, expected = dict(v['recipe']), dict(op)
            self.assertEqual(Path(actual.pop('artifact_path')).name, Path(expected.pop('artifact_path')).name)
            self.assertEqual(actual, expected)
            events = [e for e in v['events'] if e['name'] == 'RemoveKnot']
            self.assertEqual([e['result'] for e in events], ['Failure' if 'tensor' in row['id'] else 'Success'])
            self.assertEqual(v['success'], 'tensor' not in row['id'])
            path = Path(op['artifact_path'])
            self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(), capture['provenance']['fixtures'][path.name])
        for name, digest in capture['provenance']['sources'].items():
            data=(HERE/name).read_bytes()
            if hashlib.sha256(data).hexdigest()!=digest and name in('rhino_worker.py','structural_sdk.py'):
                data=gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(data).hexdigest(), digest, name)

    def test_smooth_periodic_sources_and_successful_removals(self):
        from .generate_remove_knot_smooth_periodic import request as smooth
        q = smooth()
        capture = json.loads((HERE / 'observations/remove_knot_smooth_periodic.json').read_text())
        self.assertEqual(len(q['operations']), 2)
        self.assertEqual([o['id'] for o in q['operations']], [r['id'] for r in capture['results']])
        for op, row in zip(q['operations'], capture['results']):
            v = row['value']
            self.assertEqual(v['recipe']['id'], op['id'])
            self.assertEqual(v['before'][0]['geometry']['faces'], 1)
            self.assertTrue(v['success'])
            self.assertEqual([e['result'] for e in v['events'] if e['name']=='RemoveKnot'], ['Success'])
            self.assertEqual(v['before'][0]['geometry']['surfaces'][0]['degree'], [3,3])
            path=Path(op['artifact_path'])
            self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(), capture['provenance']['fixtures'][path.name])
        for name, digest in capture['provenance']['sources'].items():
            data=(HERE/name).read_bytes()
            if hashlib.sha256(data).hexdigest()!=digest and name in('rhino_worker.py','structural_sdk.py'):
                data=gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(data).hexdigest(), digest, name)
