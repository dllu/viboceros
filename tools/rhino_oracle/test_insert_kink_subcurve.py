"""Bounded temporary getters, real owned input, and retained native evidence."""
import copy,hashlib,json,tempfile
from pathlib import Path
from unittest import TestCase,mock
from .generate_insert_kink_subcurve import request
from .insert_kink_subcurve_probe import validate
from .insert_kink_subcurve_input import InsertKinkSubcurvePicker
from .structural_sdk import stage_helpers
HERE=Path(__file__).parent
class InsertKinkSubcurve(TestCase):
    def test_bounded_sources_and_staging(self):
        q=request();self.assertEqual(len(q['operations']),128)
        with tempfile.TemporaryDirectory()as d:
            stage_helpers(q,HERE,Path(d))
            self.assertEqual({p.name for p in Path(d).iterdir()},{'insert_kink_subcurve_probe.py','insert_kink_probe.py','join_probe.py','merge_edges_probe.py','shrink_face_input.py','snap_environment.py'})
            for p in Path(d).iterdir():self.assertEqual(p.read_bytes(),(HERE/p.name).read_bytes())
    def test_invalid_getter_rejected_before_launch(self):
        base=request()['operations'][0]
        for getter in [None,{},dict(start=True),dict(start=-1.),dict(start=float('nan')),dict(start=.2,lengths=[True]),dict(start=.2,lengths=[float('inf')]),dict(start=.2,lengths=['bad']),dict(start=.2,lengths=[1]*5),dict(start=.2,end=.8,lengths=[1.]),dict(start=.2,unlock=True),dict(start=.2,unknown=True)]:
            op=copy.deepcopy(base);op['getter']=getter
            with self.subTest(getter=getter),self.assertRaises(ValueError):validate(op)
    def test_owned_driver_cancels_only_the_owned_getter_and_respects_completion(self):
        q=request();picker=InsertKinkSubcurvePicker(q)
        with tempfile.TemporaryDirectory()as d,mock.patch('subprocess.run')as send,mock.patch('time.sleep'):
            picker.job=Path(d)
            (picker.job/'kink-finished-line_cancel_getter.json').write_text(json.dumps('line_cancel_getter'))
            self.assertTrue(picker.send_input('@subcurve-direction:line_cancel_getter','1','1','owned'))
            self.assertEqual(send.call_args.args[0],['xdotool','windowactivate','--sync','owned','key','--clearmodifiers','Escape','Escape'])
            self.assertEqual(send.call_count,1)
            self.assertTrue(picker.send_input('@subcurve-direction:cubic_cancel_getter','1','1','owned'))
            self.assertEqual(send.call_count,3)
            self.assertTrue((picker.job/'kink-returned-cubic_cancel_getter.json').exists())
        bad=copy.deepcopy(q);bad['operations'].append(copy.deepcopy(bad['operations'][0]))
        with self.assertRaises(ValueError):InsertKinkSubcurvePicker(bad)
    def test_real_keyboard_toggle_and_escape_capture(self):
        r=json.loads((HERE/'observations/insert_kink_subcurve_keys.json').read_text())
        rows={v['id']:v['value']for v in r['results']}
        v=rows['line_toggle_unlock'];self.assertEqual([i['token']for i in v['input_trace']['typed']][:2],['D','D'])
        self.assertIn('Direction=Locked',' '.join(v['input_trace']['prompts']));self.assertTrue(v['success'])
        self.assertFalse(rows['line_cancel_getter']['success'])
        for name,digest in r['retained_fixture_sha256'].items():self.assertEqual(hashlib.sha256((HERE/'fixtures/insert_kink_subcurve_keys'/name).read_bytes()).hexdigest(),digest)
    def test_native_cases_sources_motion_and_provenance(self):
        capture=json.loads((HERE/'observations/insert_kink_subcurve.json').read_text());q=request()
        self.assertEqual([r['id']for r in capture['results']],[o['id']for o in q['operations']])
        for row,op in zip(capture['results'],q['operations']):
            v=row['value'];actual=dict(v['recipe']);expected=dict(op)
            self.assertEqual(Path(actual.pop('artifact_path')).name,Path(expected.pop('artifact_path')).name);self.assertEqual(actual,expected)
            self.assertEqual([e['name']for e in v['events']].count('InsertKink'),1,row['id'])
            if 'lock_hover'in op['getter']:
                self.assertEqual(len(v['input_trace']['cursor']),2);self.assertEqual(len(v['input_trace']['hover']),3);self.assertIn('Direction=Locked',v['history'])
            if op['getter'].get('cancel_getter'):self.assertIn([x['token']for x in v['input_trace']['typed']][-2:],[['Escape','Escape'],['Escape','_Enter']])
            path=Path(op['artifact_path']);self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(),capture['provenance']['fixtures'][path.name])
        for name,digest in capture['provenance']['sources'].items():
            # Dispatch evolves; the recorded hashes describe the historical
            # capture. Its producer, inputs and geometry helpers remain exact.
            if name in ('rhino_worker.py','structural_sdk.py'):continue
            self.assertEqual(hashlib.sha256((HERE/name).read_bytes()).hexdigest(),digest,name)
