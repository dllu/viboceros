"""Native command completion, saved-view provenance and repeated row picks."""
import copy
import gzip
import hashlib
import json
import tempfile
from collections import Counter
from pathlib import Path
from unittest import TestCase
from .generate_sel_uv import request
from .sel_uv_probe import validate
from .structural_sdk import stage_helpers

HERE = Path(__file__).parent


class SelectUVTests(TestCase):
    def test_complete_native_capture_and_provenance(self):
        q = request()
        path = HERE / 'fixtures/select_uv_rows.json'
        saved = json.loads(path.read_text())
        for a, b in zip(q['operations'], saved['operations']):
            self.assertEqual(Path(a.pop('artifact_path')).name, Path(b.pop('artifact_path')).name)
        self.assertEqual(q, saved)
        r = json.loads((HERE / 'observations/select_uv_rows.json').read_text())
        self.assertEqual(len(r['results']), len(q['operations']))
        self.assertEqual([o['id'] for o in q['operations']], [o['id'] for o in r['results']])
        digest = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
        self.assertEqual(digest(path), r['provenance']['request_sha256'])
        results = Counter()
        for row in r['results']:
            v = row['value']
            self.assertTrue(v['redo_retained'], row['id'])
            for state in [v] + ([v['again']] if 'again' in v else []):
                event = [e for e in state['events'] if e['name'] == 'SelUV']
                self.assertEqual(len(event), 1)
                self.assertEqual(event[0]['objects'], state['after'])
                if state is v:
                    cancelled = v['recipe'].get('cancel') or v['recipe'].get('cancel_surface')
                    self.assertEqual(event[0]['result'], 'Cancel' if cancelled else 'Success')
                if state is v: results[event[0]['result']] += 1
                self.assertEqual([(g['index'], g['point']) for g in state['after']['grips']],
                                 [(g['index'], g['point']) for g in state['after_script']['grips']])
        self.assertEqual(results['Cancel'], 7)
        for name, expected in r['provenance']['sources'].items():
            data = (HERE / name).read_bytes()
            if hashlib.sha256(data).hexdigest() != expected and name in ('rhino_worker.py', 'structural_sdk.py'):
                data = gzip.decompress((HERE / 'observations/source_snapshots' / (expected + '.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(data).hexdigest(), expected, name)
        for name, expected in r['provenance']['fixtures'].items():
            self.assertEqual(digest(HERE / 'fixtures/select_uv_rows' / name), expected)

    def test_standalone_staging_and_rejected_inputs(self):
        q = request()
        with tempfile.TemporaryDirectory() as d:
            stage_helpers(q, HERE, Path(d))
            self.assertEqual({p.name for p in Path(d).iterdir()}, {
                'sel_uv_probe.py','sel_uv_input.py','grip_navigation_probe.py',
                'grip_transform_probe.py','join_probe.py','merge_edges_probe.py'})
            for p in Path(d).iterdir(): self.assertEqual(p.read_bytes(), (HERE / p.name).read_bytes())
        base = q['operations'][0]
        for patch in [dict(family='box'),dict(actions=['Direction=Both _Delete']),
                      dict(actions=[dict(grip=True)]),dict(actions=[dict(world=[0,0,float('nan')])]),
                      dict(cancel=1),dict(selected=[True]),dict(next_actions=['HideOthers=Maybe'])]:
            with self.subTest(patch=patch), self.assertRaises(ValueError): validate(dict(copy.deepcopy(base), **patch))

    def test_memory_cancellation_and_live_flags(self):
        rows = {row['id']:row['value'] for row in json.loads((HERE / 'observations/select_uv_rows.json').read_text())['results']}
        picks = lambda v: [g['index'] for g in v['after']['grips'] if g['selected']]
        self.assertEqual(picks(rows['toggle_Both']), [1,3,4,5,7])
        self.assertEqual(picks(rows['direction_memory']['again']), [3,4,5])
        self.assertEqual(picks(rows['cancel_memory']['again']), [0,1,2,3,6])
        self.assertEqual(picks(rows['managed_cancel']), [1,4,7])
        for name in ['managed_hide','managed_cancel','managed_both']:
            trace = rows[name]['input_trace']
            self.assertTrue(any('HideOthers=Yes' in t['prompt'] for t in trace))
            self.assertTrue(any(t['picks'] for t in trace))
            self.assertTrue(all(t['visible'] and t['peer_visible'] and not t['hidden'] for t in trace))
        for name in ['preselected_0','preselected_1','preselected_seed']:
            self.assertTrue(rows[name]['after']['selected'])
            self.assertFalse(rows[name]['after_script']['selected'])
            self.assertTrue(rows[name]['after_script']['peer_selected'])

    def test_comparison_keeps_complete_states_and_checks_native_completion(self):
        from .sel_uv_comparison import canonical
        from .client import OracleProtocolError, _compare_value
        row=json.loads((HERE / 'observations/select_uv_rows.json').read_text())['results'][0]
        op=row['value']['recipe']; native=row['value']
        expected=canonical(op,native,native=True)
        self.assertEqual(canonical(op,expected,native=False),expected)
        for key in ['visible','selected','peer_visible','peer_selected','grips_on']:
            changed=copy.deepcopy(expected); changed['after'][key]=not changed['after'][key]
            differences=[]; _compare_value('state',expected,changed,1e-8,1e-12,differences)
            self.assertTrue(differences,key)
        for change in ['event','index','extra']:
            changed=copy.deepcopy(native)
            if change=='event': changed['events']=[]
            elif change=='index': changed['after']['grips'][0]['index']=.01
            else:
                changed=copy.deepcopy(expected);changed['unrecorded']=True
            with self.assertRaises(OracleProtocolError):canonical(op,changed,native=change!='extra')
