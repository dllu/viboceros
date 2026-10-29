"""Complete public command, getter, visibility and geometry witnesses."""
import copy
import gzip
import hashlib
import json
import tempfile
from collections import Counter
from pathlib import Path
from unittest import TestCase
from .generate_grip_visibility import request
from .grip_visibility_probe import validate
from .structural_sdk import stage_helpers

HERE = Path(__file__).parent


class GripVisibilityTests(TestCase):
    def test_complete_corpus_and_exact_provenance(self):
        q = request()
        path = HERE / 'fixtures/grip_visibility.json'
        saved = json.loads(path.read_text())
        for a, b in zip(q['operations'], saved['operations']):
            self.assertEqual(Path(a.pop('artifact_path')).name, Path(b.pop('artifact_path')).name)
        self.assertEqual(q, saved)
        r = json.loads((HERE / 'observations/grip_visibility.json').read_text())
        self.assertEqual(len(r['results']), 39)
        self.assertEqual([o['id'] for o in q['operations']], [o['id'] for o in r['results']])
        digest = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
        self.assertEqual(digest(path), r['provenance']['request_sha256'])
        steps = [s for row in r['results'] for s in row['value']['steps']]
        self.assertEqual(len(steps), 1219)
        self.assertEqual(Counter(s['result'] for s in steps), dict(Success=1049, Cancel=129, Nothing=41))
        self.assertEqual(sum(row['value']['redo_retained'] for row in r['results']), 31)
        for name, expected in r['provenance']['sources'].items():
            data = (HERE / name).read_bytes()
            if hashlib.sha256(data).hexdigest() != expected and name in ('rhino_worker.py', 'structural_sdk.py', 'hide_pt_input.py'):
                data = gzip.decompress((HERE / 'observations/source_snapshots' / (expected + '.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(data).hexdigest(), expected, name)
        for name, expected in r['provenance']['fixtures'].items():
            self.assertEqual(digest(HERE / 'fixtures/grip_visibility' / name), expected)
        for row in r['results']:
            for step in row['value']['steps']:
                if step['command'] not in ('Move', 'Undo', 'Redo', 'PointsOn', 'PointsOff', 'EditPtOn'):
                    for before, after in zip(step['before']['sources'], step['after']['sources']):
                        self.assertEqual([(g['index'], g['point']) for g in before['grips']],
                                         [(g['index'], g['point']) for g in after['grips']], row['id'])
                        if 'curve' in before: self.assertEqual(before['curve'], after['curve'], row['id'])

    def test_standalone_staging_and_closed_input_recipes(self):
        q = request()
        with tempfile.TemporaryDirectory() as d:
            stage_helpers(q, HERE, Path(d))
            self.assertEqual({p.name for p in Path(d).iterdir()}, {
                'grip_visibility_probe.py', 'hide_pt_input.py', 'grip_navigation_probe.py',
                'grip_transform_probe.py', 'join_probe.py', 'merge_edges_probe.py'})
            for p in Path(d).iterdir(): self.assertEqual(p.read_bytes(), (HERE / p.name).read_bytes())
        base = q['operations'][0]
        for patch in [dict(id='bad\n_Delete'), dict(sources=[True]), dict(actions=[]),
                      dict(actions=[dict(command='HidePt _Delete')]),
                      dict(actions=[dict(command='HidePt', pick='all _Delete')]),
                      dict(actions=[dict(command='ShowPt', pick='all')]),
                      dict(actions=[dict(command='HidePt', cancel=1)]),
                      dict(actions=[dict(command='HidePt', parents=1)]),
                      dict(actions=[dict(command='HidePt', seed='none')]),
                      dict(actions=[dict(command='HidePt', extra=True)])]:
            with self.subTest(patch=patch), self.assertRaises(ValueError):
                validate(dict(copy.deepcopy(base), **patch))

    def test_native_getter_completion_and_hidden_logical_selection(self):
        rows = {r['id']: r['value'] for r in json.loads((HERE / 'observations/grip_visibility.json').read_text())['results']}
        steps = [s for v in rows.values() for s in v['steps']]
        getters = Counter((s['command'], s['getter']['opened'], s['getter']['input']) for s in steps if s['getter'])
        self.assertEqual(getters, {
            ('HidePt', False, None): 215, ('HidePt', True, '_Cancel'): 102,
            ('HidePt', True, '_SelPt _Cancel'): 25, ('HidePt', True, '_SelPt _Enter'): 25,
            ('Move', False, None): 8, ('Move', True, '_Cancel'): 2})
        for step in steps:
            if step['getter'] and step['getter']['opened']:
                self.assertEqual(step['result'], 'Cancel' if step['getter']['input'].endswith('_Cancel') else 'Success')
        picks = lambda s: [g['index'] for g in s['sources'][0]['grips'] if g['selected']]
        hidden = lambda s: [g['index'] for g in s['sources'][0]['grips'] if g['hidden']]
        mixed = rows['mixed_hidden_hide_control']['steps']
        self.assertEqual(picks(mixed[2]['after']), [1, 2])
        self.assertEqual(picks(mixed[3]['after']), [2])
        self.assertEqual(hidden(mixed[3]['after']), [1, 2])
        history = rows['edited_mixed_move_history_control']['steps']
        self.assertEqual(picks(history[3]['after']), [1])
        self.assertEqual(hidden(history[3]['after']), [2])
        self.assertEqual(picks(history[4]['after']), [1, 2])
        self.assertEqual(hidden(history[4]['after']), [])

    def test_comparison_checks_every_raw_state_flag_identity_and_result(self):
        from .client import _compare_value
        value = json.loads((HERE / 'observations/grip_visibility.json').read_text())['results'][0]['value']
        for name in ['hidden', 'selectable', 'selected', 'index', 'point', 'result', 'getter', 'curve']:
            changed = copy.deepcopy(value)
            step = changed['steps'][0]
            grip = step['after']['sources'][0]['grips'][0]
            if name in ('hidden', 'selectable', 'selected'): grip[name] = not grip[name]
            elif name == 'index': grip[name] += 1
            elif name == 'point': grip[name][0] += .1
            elif name == 'result': step[name] = 'Failure'
            elif name == 'getter': step[name]['opened'] = not step[name]['opened']
            else: step['after']['sources'][0]['curve'] = {}
            differences = []
            _compare_value('state', value, changed, 1e-8, 1e-12, differences)
            self.assertTrue(differences, name)
