"""Input provenance, native getter acknowledgements, bounds and real history."""
import gzip,hashlib,json,os,subprocess,sys,tempfile
from pathlib import Path
from unittest import TestCase,mock
from .generate_array_curve_surface import request
from .structural_sdk import validate_operations,stage_helpers
from .client import OracleClient,OracleProtocolError
HERE=Path(__file__).resolve().parent

def source_bytes(name,sha):
    data=(HERE/name).read_bytes()
    if hashlib.sha256(data).hexdigest()!=sha:data=gzip.decompress((HERE/'observations/source_snapshots'/(sha+'.gz')).read_bytes())
    return data

class ArrayCurveSurfaceTests(TestCase):
    def test_reproducible_bounded_request_source_closure_and_staging(self):
        q=request();self.assertEqual(q,json.loads((HERE/'fixtures/array_curve_surface.json').read_text()))
        r=json.loads((HERE/'observations/array_curve_surface.json').read_text());self.assertEqual(len(r['results']),44)
        for name,sha in r['provenance']['sources'].items():self.assertEqual(hashlib.sha256(source_bytes(name,sha)).hexdigest(),sha,name)
        with tempfile.TemporaryDirectory()as folder:
            stage_helpers(q,HERE,Path(folder))
            for name in ['array_curve_surface_probe.py','number_token.py','conic_getter_input.py','managed_mouse_callback.py','snap_environment.py','join_probe.py','merge_edges_probe.py']:
                self.assertEqual((Path(folder)/name).read_bytes(),(HERE/name).read_bytes())
            subprocess.run([sys.executable,'-I','-c','import sys,json;sys.path.insert(0,sys.argv[1]);import array_curve_surface_probe;array_curve_surface_probe.validate(json.loads(sys.argv[2]))',folder,json.dumps(q['operations'][0])],check=True,capture_output=True)
    def test_all_getters_completed_with_acknowledged_picks_and_real_undo(self):
        q=request();r=json.loads((HERE/'observations/array_curve_surface.json').read_text());contacts=0
        for op,row in zip(q['operations'],r['results']):
            with self.subTest(id=op['id']):
                self.assertEqual(op['id'],row['id']);self.assertNotIn('error',row);v=row['value'];t=v['input_trace'];self.assertEqual(t['errors'],[]);self.assertEqual(t['completed'],len(v['actions']));self.assertEqual(t['expected'],t['completed']);self.assertTrue(v['guides_unchanged']);self.assertNotIn('Unknown command',v['history'])
                for c in t.get('clicks',[]):
                    contacts+=1;self.assertTrue(c['motion_acknowledged']and c['click_acknowledged']);self.assertEqual(c['aim'],op['pick_point']);self.assertEqual(len(c['projected_point']),3)
                before=v['before']['objects'];after=v['after_script']['objects'];outputs=[o for o in after if o['role']=='output']
                for phase in ['after_script','undo','redo']:
                    for src in [o for o in v[phase]['objects']if o['role']=='source']:self.assertEqual(src['selected'],not op['postselect']and op['subcurve']is None)
                if outputs:
                    self.assertIn('Undoing ArrayCrvOnSrf',v['undo_history']);self.assertIn('Redoing ArrayCrvOnSrf',v['undo_history']);self.assertEqual(v['after_script'],v['redo']);self.assertEqual(len(v['undo']['objects']),len(before))
                else:self.assertIn('Nothing to undo',v['undo_history'])
        self.assertEqual(contacts,3)
    def test_rejected_controls_do_not_launch_rhino(self):
        op=request()['operations'][0]
        for patch in [dict(extra=True),dict(grouped=1),dict(source_count=0),dict(source_count=100),dict(value=1000001),dict(scene='Unknown'),dict(subcurve=[0,0]),dict(pick_point=[float('nan'),0,0]),dict(seed_names=['x'*65]),dict(mode='Sequence',value=[dict(other=1)])]:
            with self.subTest(patch=patch),self.assertRaises(ValueError):validate_operations(dict(operations=[dict(op,**patch)]))
        q=request()
        with mock.patch.dict(os.environ,dict(DISPLAY=':301',VIBOCEROS_ORACLE_HEADLESS=':301'),clear=True),mock.patch('tools.rhino_oracle.client._run_logged')as launch:
            for scheme,req in [(None,dict(q,operations=[op])),('VibocerosOracleArrayUnit',dict(q,iterations=2)),('VibocerosOracleArrayUnit',dict(q,operations=[op,op])),('VibocerosOracleArrayUnit',dict(q,operations=q['operations']*2))]:
                with self.assertRaises((ValueError,OracleProtocolError)):OracleClient(launcher=Path('/bin/true'),settings_scheme=scheme).run_rhino(req,1)
            launch.assert_not_called()
    def test_original_failed_getter_attempts_remain_available(self):
        for name,count in [('input-pilot',3),('getter-diagnostic',14),('protocol-diagnostic',16),('followup-diagnostic',13)]:
            d=json.loads(gzip.decompress((HERE.parent.parent/'docs'/('array-curve-surface-'+name+'.json.gz')).read_bytes()));self.assertEqual(len(d['request']['operations']),count)
            for name,sha in d['response']['provenance']['sources'].items():self.assertEqual(hashlib.sha256(source_bytes(name,sha)).hexdigest(),sha,name)
    def test_validation_receipt_pins_recorded_implementation_and_all_replays(self):
        receipt=json.loads((HERE.parent.parent/'docs/array-curve-surface-validation.json').read_text())
        self.assertEqual(receipt['qualified_workflows'],44)
        for path,sha in receipt['implementation_sources'].items():
            current=HERE.parent.parent/path;data=current.read_bytes()if current.exists()else b''
            if hashlib.sha256(data).hexdigest()!=sha:data=gzip.decompress((HERE/'observations/source_snapshots'/(sha+'.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(data).hexdigest(),sha,path)
        for label,rows in receipt['replay_audits'].items():
            self.assertEqual([r['id']for r in rows],[op['id']for op in request()['operations']],label)
            self.assertLessEqual(max(r['maximum_error']for r in rows),receipt['comparison']['absolute_cloud_bound'])
