"""ArraySrf capture bounds, getter gates, source receipts and real model history."""
import gzip,hashlib,json,os,subprocess,sys,tempfile
from pathlib import Path
from unittest import TestCase,mock
from .generate_array_surface import request
from .structural_sdk import validate_operations,stage_helpers
from .client import OracleClient,OracleProtocolError
HERE=Path(__file__).resolve().parent

def source(name,sha):
    data=(HERE/name).read_bytes()
    if hashlib.sha256(data).hexdigest()!=sha:data=gzip.decompress((HERE/'observations/source_snapshots'/(sha+'.gz')).read_bytes())
    return data
class ArraySurfaceTests(TestCase):
    def test_reproducible_inputs_frozen_sources_and_standalone_helpers(self):
        q=request();self.assertEqual(q,json.loads((HERE/'fixtures/array_surface.json').read_text()));r=json.loads((HERE/'observations/array_surface.json').read_text());self.assertEqual(len(r['results']),46)
        self.assertEqual(hashlib.sha256((HERE/'fixtures/array_surface.json').read_bytes()).hexdigest(),r['provenance']['request_sha256'])
        for name,sha in r['provenance']['sources'].items():self.assertEqual(hashlib.sha256(source(name,sha)).hexdigest(),sha,name)
        with tempfile.TemporaryDirectory()as folder:
            stage_helpers(q,HERE,Path(folder))
            for name in ['array_surface_probe.py','number_token.py','conic_getter_input.py','join_probe.py','merge_edges_probe.py','managed_mouse_callback.py','snap_environment.py']:self.assertEqual((Path(folder)/name).read_bytes(),(HERE/name).read_bytes())
            subprocess.run([sys.executable,'-I','-c','import sys,json;sys.path.insert(0,sys.argv[1]);import array_surface_probe;array_surface_probe.validate(json.loads(sys.argv[2]))',folder,json.dumps(q['operations'][0])],check=True,capture_output=True)
    def test_every_input_reaches_its_public_getter_and_real_history_matches_copies(self):
        r=json.loads((HERE/'observations/array_surface.json').read_text())
        for op,row in zip(request()['operations'],r['results']):
            with self.subTest(id=op['id']):
                self.assertEqual(op['id'],row['id']);self.assertNotIn('error',row);v=row['value'];t=v['input_trace'];self.assertTrue(v['target_unchanged']);self.assertEqual(t['errors'],[]);self.assertEqual(t['expected'],t['completed']);self.assertEqual(t['completed'],len(v['actions']));self.assertNotIn('Unknown command',v['history'])
                for sent in t['sent']:self.assertTrue(any(sent['prompt'].startswith(p)for p in v['action_prompts'][sent['action']]))
                self.assertTrue(all(p for prefixes in v['action_prompts']for p in prefixes))
                copies=[o for o in v['after_script']['objects']if o['role']=='output']
                if copies:self.assertIn('Undoing ArraySrf',v['undo_history']);self.assertIn('Redoing ArraySrf',v['undo_history']);self.assertEqual(v['after_script'],v['redo']);self.assertEqual(len(v['undo']['objects']),len(v['before']['objects']))
                else:self.assertIn('Nothing to undo',v['undo_history'])
                for phase in ['after_script','undo','redo']:
                    self.assertTrue(all(o['selected']==(not op['postselect'])for o in v[phase]['objects']if o['role']=='source'))
    def test_invalid_recipes_are_bounded_before_launch(self):
        q=request();op=q['operations'][0]
        for patch in [dict(extra=True),dict(scene='Unknown'),dict(counts=[0,3]),dict(counts=[True,2]),dict(counts=[1000000,2]),dict(up_point=[float('inf'),0,0]),dict(cplane=[]),dict(preview=1),dict(preview=False,edits=[dict(UNumber=3)]),dict(domains=[[0,0],[0,1]]),dict(edits=[dict(UNumber=0)]),dict(source_count=4)]:
            with self.subTest(patch=patch),self.assertRaises(ValueError):validate_operations(dict(operations=[dict(op,**patch)]))
        with mock.patch.dict(os.environ,dict(DISPLAY=':301',VIBOCEROS_ORACLE_HEADLESS=':301'),clear=True),mock.patch('tools.rhino_oracle.client._run_logged')as launch:
            for scheme,req in [(None,q),('VibocerosOracleArraySurfaceUnit',dict(q,iterations=2)),('VibocerosOracleArraySurfaceUnit',dict(q,operations=[op,op])),('VibocerosOracleArraySurfaceUnit',dict(q,operations=q['operations']*2))]:
                with self.assertRaises((ValueError,OracleProtocolError)):OracleClient(launcher=Path('/bin/true'),settings_scheme=scheme).run_rhino(req,1)
            launch.assert_not_called()
    def test_diagnostics_retain_exact_old_producers_and_the_failed_trim_recipe(self):
        for name,count in [('input-pilot',12),('protocol-pilot',40)]:
            d=json.loads(gzip.decompress((HERE.parent.parent/'docs'/('array-surface-'+name+'.json.gz')).read_bytes()));self.assertEqual(len(d['request']['operations']),count)
            for name,sha in d['response']['provenance']['sources'].items():self.assertEqual(hashlib.sha256(source(name,sha)).hexdigest(),sha,name)
        d=json.loads(gzip.decompress((HERE.parent.parent/'docs/array-surface-trim-input-diagnostic.json.gz').read_bytes()));self.assertEqual(len(d['request']['operations']),38);self.assertIsNone(d['response']);self.assertIn('degree_u',d['error'])
        for name,sha in d['sources'].items():self.assertEqual(hashlib.sha256(source(name,sha)).hexdigest(),sha,name)
    def test_validation_receipt_pins_recorded_code_and_every_workflow(self):
        receipt=json.loads((HERE.parent.parent/'docs/array-surface-validation.json').read_text());self.assertEqual(receipt['workflows'],46)
        for name,sha in receipt['implementation_sources'].items():
            path=HERE.parent.parent/name;data=path.read_bytes()if path.exists()else b''
            if hashlib.sha256(data).hexdigest()!=sha:data=gzip.decompress((HERE/'observations/source_snapshots'/(sha+'.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(data).hexdigest(),sha,name)
        for label,rows in receipt['replay_audits'].items():
            self.assertEqual([r['id']for r in rows],[op['id']for op in request()['operations']],label);self.assertLessEqual(max(r['maximum_error']for r in rows),1e-6)
