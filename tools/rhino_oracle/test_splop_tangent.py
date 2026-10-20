"""Tangent reference input receipts, isolated capture bounds and source closure."""
import gzip,hashlib,json,os,subprocess,sys,tempfile
from pathlib import Path
from unittest import TestCase,mock
from .generate_splop_tangent import request
from .structural_sdk import validate_operations,stage_helpers
from .client import OracleClient,OracleProtocolError
HERE=Path(__file__).resolve().parent

def source_bytes(name,sha):
    data=(HERE/name).read_bytes()
    if hashlib.sha256(data).hexdigest()!=sha:data=gzip.decompress((HERE/'observations/source_snapshots'/(sha+'.gz')).read_bytes())
    return data

class SplopTangentTests(TestCase):
    def test_reproducible_inputs_sources_and_standalone_staging(self):
        q=request();self.assertEqual(q,json.loads((HERE/'fixtures/splop_tangent.json').read_text()))
        r=json.loads((HERE/'observations/splop_tangent.json').read_text());self.assertEqual(len(r['results']),100)
        self.assertEqual(hashlib.sha256((HERE/'fixtures/splop_tangent.json').read_bytes()).hexdigest(),r['provenance']['request_sha256'])
        for name,sha in r['provenance']['sources'].items():self.assertEqual(hashlib.sha256(source_bytes(name,sha)).hexdigest(),sha,name)
        batches=r['provenance']['batches'];offset=0
        for batch in batches:
            self.assertEqual(batch['offset'],offset);self.assertLessEqual(len(batch['request']['operations']),64);offset+=len(batch['request']['operations'])
        self.assertEqual([op for batch in batches for op in batch['request']['operations']],q['operations'])
        with tempfile.TemporaryDirectory()as folder:
            stage_helpers(q,HERE,Path(folder))
            for name in ['splop_tangent_probe.py','circle_around_probe.py','conic_getter_input.py','managed_mouse_callback.py','snap_environment.py','curve_source_bounds.py','getter_recipe.py','join_probe.py','merge_edges_probe.py']:
                self.assertEqual((Path(folder)/name).read_bytes(),(HERE/name).read_bytes())
            subprocess.run([sys.executable,'-I','-c','import sys,json;sys.path.insert(0,sys.argv[1]);import splop_tangent_probe;splop_tangent_probe.validate(json.loads(sys.argv[2]))',folder,json.dumps(q['operations'][0])],check=True,capture_output=True)
    def test_acknowledged_curve_contacts_and_real_undo_redo(self):
        q=request();r=json.loads((HERE/'observations/splop_tangent.json').read_text());contacts=edges=0
        for op,row in zip(q['operations'],r['results']):
            with self.subTest(id=op['id']):
                self.assertEqual(op['id'],row['id']);self.assertNotIn('error',row);v=row['value'];t=v['input_trace']
                self.assertEqual(t['errors'],[]);self.assertEqual(t['completed'],len(v['driven_actions']));self.assertEqual(t['expected'],t['completed']);self.assertNotIn('Unknown command',v['history']);self.assertTrue(v['guides_unchanged'])
                expected=[i for i,a in enumerate(op['actions'])if isinstance(a,dict)and'pick'in a]
                self.assertEqual([c['action']for c in t.get('clicks',[])],expected)
                for c in t.get('clicks',[]):
                    self.assertTrue(c['motion_acknowledged']and c['click_acknowledged']);a=op['actions'][c['action']]
                    self.assertEqual(c['curve'],a['curve']);self.assertEqual(c['edge'],a.get('edge'));self.assertEqual(c['aim'],a['pick']);self.assertEqual(len(c['projected_point']),3);self.assertIsInstance(c['projected_parameter'],float);contacts+=1;edges+=int('edge'in a)
                for phase in ['before','after','after_script','undo','redo']:
                    self.assertEqual(v[phase]['target_selected'],phase=='after'and op['finish']!='CancelReference')
                    objects=v[phase]['objects'];self.assertTrue(next(o for o in objects if o['role']=='source')['selected'])
                    self.assertTrue(all(not o['selected']for o in objects if o['role']!='source'))
                    self.assertTrue(all(o['geometry']['kind']=='PointCloud'for o in objects if o['role']=='output'))
                if op['finish']=='CancelReference':
                    self.assertIn('Nothing to undo',v['undo_history']);self.assertEqual(v['before'],v['after_script']);self.assertEqual(v['before'],v['undo']);self.assertEqual(v['undo'],v['redo'])
                else:
                    self.assertIn('Undoing Splop',v['undo_history']);self.assertIn('Redoing Splop',v['undo_history']);self.assertEqual(v['before'],v['undo']);self.assertEqual(v['after_script'],v['redo'])
        self.assertGreater(contacts,100);self.assertEqual(edges,6)
    def test_bounded_scenes_reject_invalid_controls_without_launch(self):
        op=request()['operations'][0]
        for patch in [dict(copy=1),dict(rigid=None),dict(flip='No'),dict(surface='Sphere'),dict(finish='IdleCancel'),dict(finish='CancelCopies',copy=False),dict(actions=['Point']),dict(extra=True),dict(curves='bad'),dict(curves=[None]),dict(curves=[dict(type='point',point=[0,0,0])])]:
            with self.subTest(patch=patch),self.assertRaises(ValueError):validate_operations(dict(operations=[dict(op,**patch)]))
    def test_private_dedicated_capture_and_unique_bounded_ids(self):
        q=request();op=q['operations'][0]
        with mock.patch.dict(os.environ,dict(DISPLAY=':301',VIBOCEROS_ORACLE_HEADLESS=':301'),clear=True),mock.patch('tools.rhino_oracle.client._run_logged')as launch:
            for scheme,req in [(None,dict(q,operations=[op])),('VibocerosOracleSplopTangentUnit',q),('VibocerosOracleSplopTangentUnit',dict(q,iterations=2,operations=[op])),('VibocerosOracleSplopTangentUnit',dict(q,iterations=True,operations=[op])),('VibocerosOracleSplopTangentUnit',dict(q,operations=[op,op])),('VibocerosOracleSplopTangentUnit',dict(q,operations=[op,dict(op,op='splop_guided')]))]:
                with self.assertRaises((ValueError,OracleProtocolError)):OracleClient(launcher=Path('/bin/true'),settings_scheme=scheme).run_rhino(req,1)
            launch.assert_not_called()
    def test_contact_diagnostics_keep_native_sphere_and_circle_results_separate(self):
        d=json.loads(gzip.decompress((HERE.parent.parent/'docs/splop-tangent-contact-diagnostic.json.gz').read_bytes()))
        self.assertEqual(len(d['captures']),3)
        for capture in d['captures']:
            q=capture['request'];r=capture['response'];self.assertEqual(len(q['operations']),3)
            self.assertEqual(hashlib.sha256((json.dumps(q,indent=2)+'\n').encode()).hexdigest(),r['provenance']['request_sha256'])
            for row in r['results']:self.assertNotIn('error',row);self.assertEqual(row['value']['input_trace']['errors'],[])
            for name,sha in r['provenance']['sources'].items():self.assertEqual(hashlib.sha256(source_bytes(name,sha)).hexdigest(),sha,name)
            self.assertEqual(hashlib.sha256(d['scripts'][r['provenance']['driver_script']].encode()).hexdigest(),r['provenance']['driver_script_sha256'])
        q=request();r=json.loads((HERE/'observations/splop_tangent.json').read_text())
        self.assertEqual(q['operations'][-3:],d['captures'][-1]['request']['operations'])
        self.assertEqual(r['results'][-3:],d['captures'][-1]['response']['results'])
    def test_clean_pilot_remains_separate_and_reproducible(self):
        d=json.loads(gzip.decompress((HERE.parent.parent/'docs/splop-tangent-pilot.json.gz').read_bytes()));self.assertEqual(len(d['request']['operations']),6)
        for row in d['response']['results']:self.assertNotIn('error',row);self.assertEqual(row['value']['input_trace']['errors'],[])
        for name,sha in d['response']['provenance']['sources'].items():self.assertEqual(hashlib.sha256(source_bytes(name,sha)).hexdigest(),sha,name)
