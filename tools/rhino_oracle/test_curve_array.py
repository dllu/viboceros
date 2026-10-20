"""Public ArrayCrv getter delivery, capture limits and frozen evidence."""
import gzip,hashlib,json,os,subprocess,sys,tempfile
from pathlib import Path
from unittest import TestCase,mock
from .generate_curve_array import request
from .structural_sdk import validate_operations,stage_helpers
from .client import OracleClient,OracleProtocolError
HERE=Path(__file__).resolve().parent

def source(name,sha):
    data=(HERE/name).read_bytes()
    if hashlib.sha256(data).hexdigest()!=sha:data=gzip.decompress((HERE/'observations/source_snapshots'/(sha+'.gz')).read_bytes())
    return data
class CurveArrayTests(TestCase):
    def test_reproducible_input_and_source_closure(self):
        path=HERE/'fixtures/curve_array_input.json';q=request();self.assertEqual(q,json.loads(path.read_text()));r=json.loads((HERE/'observations/curve_array_input.json').read_text());self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(),r['provenance']['request_sha256'])
        for name,sha in r['provenance']['sources'].items():self.assertEqual(hashlib.sha256(source(name,sha)).hexdigest(),sha,name)
        with tempfile.TemporaryDirectory()as folder:
            stage_helpers(q,HERE,Path(folder))
            for name in ['curve_array_probe.py','number_token.py','conic_getter_input.py','join_probe.py','merge_edges_probe.py','managed_mouse_callback.py','snap_environment.py']:self.assertEqual((Path(folder)/name).read_bytes(),(HERE/name).read_bytes())
            subprocess.run([sys.executable,'-I','-c','import sys,json;sys.path.insert(0,sys.argv[1]);import curve_array_probe;curve_array_probe.validate(json.loads(sys.argv[2]))',folder,json.dumps(q['operations'][0])],check=True,capture_output=True)
    def test_every_getter_motion_click_and_model_history(self):
        r=json.loads((HERE/'observations/curve_array_input.json').read_text());self.assertEqual(len(r['results']),62)
        for op,row in zip(request()['operations'],r['results']):
            with self.subTest(id=op['id']):
                self.assertEqual(op['id'],row['id']);self.assertNotIn('error',row);v=row['value'];t=v['input_trace'];self.assertTrue(v['guide_unchanged']);self.assertEqual(t['errors'],[]);self.assertEqual(t['expected'],t['completed']);self.assertEqual(t['completed'],len(v['actions']));self.assertNotIn('Unknown command',v['history'])
                for sent in t['sent']:self.assertTrue(any(sent['prompt'].startswith(p)for p in v['action_prompts'][sent['action']]))
                self.assertTrue(all(prefixes and all(prefixes)for prefixes in v['action_prompts']))
                for click in t.get('clicks',[]):self.assertTrue(click['motion_acknowledged']);self.assertTrue(click['click_acknowledged'])
                outputs=[o for o in v['after_script']['objects']if o['role']=='output']
                if outputs:self.assertIn('Undoing ArrayCrv',v['undo_history']);self.assertIn('Redoing ArrayCrv',v['undo_history']);self.assertEqual(v['after_script'],v['redo']);self.assertEqual(len(v['undo']['objects']),len(v['before']['objects']))
                else:self.assertIn('Nothing to undo',v['undo_history'])
                for phase in ['after_script','undo','redo']:
                    self.assertTrue(all(o['selected']==(not op['postselect']and op['subcurve']is None)for o in v[phase]['objects']if o['role']=='source'))
    def test_invalid_recipes_fail_before_launch(self):
        q=request();op=q['operations'][0]
        for patch in [dict(extra=True),dict(value=17),dict(value=True),dict(source_count=4),dict(pick_point=[float('inf'),0,0]),dict(cplane=[]),dict(path='Unknown'),dict(path='Edge'),dict(path='Edge',pick_point=[1,0,0],reverse_path=True),dict(subcurve=[.5,.5]),dict(input='Unknown'),dict(edits=[dict(Items=0)]),dict(finish='Unknown')]:
            with self.subTest(patch=patch),self.assertRaises(ValueError):validate_operations(dict(operations=[dict(op,**patch)]))
        with mock.patch.dict(os.environ,dict(DISPLAY=':301',VIBOCEROS_ORACLE_HEADLESS=':301'),clear=True),mock.patch('tools.rhino_oracle.client._run_logged')as launch:
            for scheme,req in [(None,q),('VibocerosOracleCurveArrayUnit',dict(q,iterations=2)),('VibocerosOracleCurveArrayUnit',dict(q,operations=[op,op])),('VibocerosOracleCurveArrayUnit',dict(q,operations=q['operations']*2))]:
                with self.assertRaises((ValueError,OracleProtocolError)):OracleClient(launcher=Path('/bin/true'),settings_scheme=scheme).run_rhino(req,1)
            launch.assert_not_called()
    def test_failed_protocols_and_corner_diagnostics_retain_their_producers(self):
        for label,count in [('input',9),('option',10),('protocol',52),('placement',56)]:
            d=json.loads(gzip.decompress((HERE.parent.parent/'docs'/('curve-array-'+label+'-pilot.json.gz')).read_bytes()));self.assertEqual(len(d['request']['operations']),count)
            for name,sha in d['response']['provenance']['sources'].items():self.assertEqual(hashlib.sha256(source(name,sha)).hexdigest(),sha,name)
        d=json.loads(gzip.decompress((HERE.parent.parent/'docs/curve-array-option-pilot.json.gz').read_bytes()));self.assertIn('Number must be bigger than or equal to 2',d['response']['results'][1]['value']['history']);self.assertEqual(len(d['response']['results'][5]['value']['frame_audit']),5)
    def test_validation_receipt_pins_implementation_and_each_replay(self):
        receipt=json.loads((HERE.parent.parent/'docs/curve-array-validation.json').read_text());self.assertEqual(receipt['workflows'],62)
        for name,sha in receipt['artifacts'].items():self.assertEqual(hashlib.sha256((HERE.parent.parent/name).read_bytes()).hexdigest(),sha,name)
        for name,sha in receipt['implementation_sources'].items():
            path=HERE.parent.parent/name;data=path.read_bytes()if path.exists()else b''
            if hashlib.sha256(data).hexdigest()!=sha:data=gzip.decompress((HERE/'observations/source_snapshots'/(sha+'.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(data).hexdigest(),sha,name)
        for label,rows in receipt['replay_audits'].items():
            self.assertEqual([r['id']for r in rows],[op['id']for op in request()['operations']],label);self.assertEqual(sum(r['qualification']=='qualified'for r in rows),57);self.assertLessEqual(max(r['maximum_error']for r in rows if r['qualification']=='qualified'),1e-6)
