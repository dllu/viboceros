"""Fixed public command recipes and source-pinned selection/history evidence."""
import gzip,hashlib,json,os,subprocess,sys,tempfile
from pathlib import Path
from unittest import TestCase,mock
from .client import OracleClient,OracleProtocolError
from .surface_offset_command_probe import request,through_request,select_request,curved_request,validate,validate_request
from .structural_sdk import stage_helpers
HERE=Path(__file__).resolve().parent
class SurfaceOffsetCommandTests(TestCase):
    def test_general_surface_commands_preserve_owned_parents_and_one_history_step(self):
        from .surface_offset_command_probe import general_request
        path=HERE/'fixtures/surface_offset_general_ends.json';q=json.loads(path.read_text())
        self.assertEqual(q,general_request());validate_request(q)
        r=json.loads((HERE/'observations/surface_offset_general_ends.json').read_text())
        self.assertEqual(len(r['results']),4)
        self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(),r['provenance']['request_sha256'])
        for name,sha in r['provenance']['sources'].items():
            data=(HERE/name).read_bytes()
            if hashlib.sha256(data).hexdigest()!=sha:data=gzip.decompress((HERE/'observations/source_snapshots'/(sha+'.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(data).hexdigest(),sha,name)
        for op,row in zip(q['operations'],r['results']):
            self.assertEqual(op['id'],row['id']);self.assertNotIn('error',row)
            v=row['value'];self.assertTrue(v['success']);self.assertEqual(v['input_trace']['errors'],[])
            self.assertEqual(v['input_trace']['completed'],len(op['actions']))
            self.assertEqual(v['before'],v['undo']);self.assertEqual(v['after_script'],v['redo'])
            self.assertEqual(v['after_script'][:2],v['before']);self.assertEqual(len(v['after_script']),3)
            self.assertFalse(v['after_script'][2]['selected']);self.assertEqual(v['after_script'][2]['layer'],'Surface offset output');self.assertIsNone(v['after_script'][2]['name'])
    def test_curved_end_command_evidence_and_source_closure(self):
        path=HERE/'fixtures/surface_offset_curved_ends.json';q=json.loads(path.read_text())
        self.assertEqual(q,curved_request());validate_request(q)
        r=json.loads((HERE/'observations/surface_offset_curved_ends.json').read_text())
        self.assertEqual(len(r['results']),8)
        self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(),r['provenance']['request_sha256'])
        for name,sha in r['provenance']['sources'].items():
            data=(HERE/name).read_bytes()
            if hashlib.sha256(data).hexdigest()!=sha:data=gzip.decompress((HERE/'observations/source_snapshots'/(sha+'.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(data).hexdigest(),sha,name)
        for op,row in zip(q['operations'],r['results']):
            self.assertEqual(op['id'],row['id']);self.assertNotIn('error',row)
            v=row['value'];self.assertTrue(v['success']);self.assertEqual(v['input_trace']['errors'],[])
            self.assertEqual(v['input_trace']['completed'],len(op['actions']))
            self.assertEqual(v['before'],v['undo']);self.assertEqual(v['after_script'],v['redo'])
            for phase in ['after_script','undo','redo']:self.assertEqual(v[phase][:2],v['before'])
            self.assertEqual(len(v['after_script'])-2,2 if op['case']=='curved_diagonal'else 1)
        for patch in [dict(actions=['Delete']),dict(action_prompts=[['Command']]),dict(case='curved_unknown')]:
            with self.assertRaises(ValueError):validate(dict(q['operations'][0],**patch))
    def test_selected_points_evidence_order_cancellation_and_source_closure(self):
        path=HERE/'fixtures/surface_offset_select_point.json';q=json.loads(path.read_text())
        self.assertEqual(q,select_request());validate_request(q)
        r=json.loads((HERE/'observations/surface_offset_select_point.json').read_text())
        self.assertEqual(len(r['results']),8)
        self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(),r['provenance']['request_sha256'])
        for name,sha in r['provenance']['sources'].items():
            data=(HERE/name).read_bytes()
            if hashlib.sha256(data).hexdigest()!=sha:data=gzip.decompress((HERE/'observations/source_snapshots'/(sha+'.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(data).hexdigest(),sha,name)
        for op,row in zip(q['operations'],r['results']):
            self.assertEqual(op['id'],row['id']);self.assertNotIn('error',row)
            v=row['value'];success=op['case']not in('through_select_cancel','through_select_empty')
            self.assertEqual(v['success'],success);self.assertEqual(v['input_trace']['errors'],[])
            self.assertEqual(v['input_trace']['completed'],len(op['actions']))
            parents=[dict(x,selected=False)for x in v['before']]
            self.assertEqual(v['undo'],parents);self.assertEqual(v['redo'],v['after_script'])
            self.assertEqual(v['after_script'][:len(parents)],parents)
            outputs=v['after_script'][len(parents):]
            self.assertEqual(bool(outputs),success and op['case']!='through_select_all_on_source')
            for output in outputs:
                self.assertFalse(output['selected']);self.assertEqual(output['layer'],'Surface offset output');self.assertIsNone(output['name'])
        for body in [dict(q,operations=[dict(q['operations'][0],actions=['Delete'])]),dict(q,iterations=2)]:
            with self.assertRaises(ValueError):validate_request(body)
    def test_independent_sdk_queries_preserve_the_captured_geometry(self):
        from .generate_surface_through_verify import request as verify_request
        path=HERE/'fixtures/surface_offset_through_verify.json';q=json.loads(path.read_text())
        self.assertEqual(q,verify_request())
        r=json.loads((HERE/'observations/surface_offset_through_verify.json').read_text())
        self.assertEqual(len(r['results']),9)
        self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(),r['provenance']['request_sha256'])
        self.assertEqual(hashlib.sha256((HERE/'observations/surface_offset_through_point.json').read_bytes()).hexdigest(),r['provenance']['capture_sha256'])
        for name,sha in r['provenance']['sources'].items():
            data=(HERE/name).read_bytes()
            if hashlib.sha256(data).hexdigest()!=sha:data=gzip.decompress((HERE/'observations/source_snapshots'/(sha+'.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(data).hexdigest(),sha,name)
        for op,row in zip(q['operations'],r['results']):self.assertEqual(op['id'],row['id']);self.assertNotIn('error',row)
        distances={r['id']:r['value']['distance']for r in r['results']}
        self.assertGreater(distances['bezier_normal_2'],.0038)
        self.assertGreater(distances['cylinder_pick_1'],3e-5)
        self.assertLess(distances['bezier_picked_point'],1e-6)
        self.assertLess(distances['sphere_pick'],1e-6)
    def test_through_point_evidence_and_source_closure(self):
        path=HERE/'fixtures/surface_offset_through_point.json';q=json.loads(path.read_text())
        self.assertEqual(q,through_request());validate_request(q)
        r=json.loads((HERE/'observations/surface_offset_through_point.json').read_text())
        self.assertEqual(len(r['results']),16)
        self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(),r['provenance']['request_sha256'])
        for name,sha in r['provenance']['sources'].items():
            data=(HERE/name).read_bytes()
            if hashlib.sha256(data).hexdigest()!=sha:data=gzip.decompress((HERE/'observations/source_snapshots'/(sha+'.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(data).hexdigest(),sha,name)
        for op,row in zip(q['operations'],r['results']):
            self.assertEqual(op['id'],row['id']);self.assertNotIn('error',row)
            v=row['value'];self.assertEqual(v['success'],op['case']!='through_cancel')
            self.assertEqual(v['input_trace']['errors'],[]);self.assertEqual(v['input_trace']['completed'],len(op['actions']))
            for phase in ['after_script','undo','redo']:self.assertEqual(v[phase][:2],v['before'])
            self.assertEqual(v['after_script'],v['redo']);self.assertEqual(v['before'],v['undo'])
            outputs=v['after_script'][2:]
            if op['case']=='through_on_source':self.assertEqual(outputs,[])
            else:self.assertGreater(len(outputs),0)
            if op['case']=='through_number':self.assertEqual(len(outputs),1)
            for output in outputs:
                self.assertFalse(output['selected']);self.assertEqual(output['layer'],'Surface offset output');self.assertIsNone(output['name'])
            self.assertIn('surface_definition',v)
    def test_command_evidence_and_complete_source_closure(self):
        path=HERE/'fixtures/surface_offset_command.json';q=json.loads(path.read_text())
        self.assertEqual(q,request());validate_request(q)
        r=json.loads((HERE/'observations/surface_offset_command.json').read_text())
        self.assertEqual(len(r['results']),8)
        self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(),r['provenance']['request_sha256'])
        for name,sha in r['provenance']['sources'].items():
            data=(HERE/name).read_bytes()
            if hashlib.sha256(data).hexdigest()!=sha:data=gzip.decompress((HERE/'observations/source_snapshots'/(sha+'.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(data).hexdigest(),sha,name)
        for op,row in zip(q['operations'],r['results']):
            self.assertEqual(op['id'],row['id']);self.assertNotIn('error',row)
            v=row['value'];self.assertTrue(v['success']);trace=v['input_trace']
            self.assertEqual(trace['errors'],[]);self.assertEqual(trace['completed'],len(op['actions']))
            for phase in ['after_script','undo','redo']:
                self.assertEqual(v[phase][:2],v['before'])
            self.assertEqual(v['after_script'],v['redo']);self.assertEqual(v['before'],v['undo'])
            for output in v['after_script'][2:]:
                self.assertFalse(output['selected']);self.assertEqual(output['layer'],'Surface offset output');self.assertIsNone(output['name'])
            event=next(e for e in v['events']if e['name']=='OffsetCrvOnSrf')
            self.assertEqual(event['result'],'Success');self.assertEqual(event['objects'],v['after'])
        # Number continues the untrimmed offset, then clips each copy: the hole
        # splits only the first copy, without duplicating subsequent copies.
        self.assertEqual(len(r['results'][-1]['value']['after_script']),6)
    def test_staged_command_probe_imports_without_the_application(self):
        with tempfile.TemporaryDirectory()as folder:
            stage_helpers(request(),HERE,Path(folder))
            self.assertEqual((Path(folder)/'surface_offset_command_probe.py').read_bytes(),(HERE/'surface_offset_command_probe.py').read_bytes())
            subprocess.run([sys.executable,'-I','-c','import sys,json;sys.path.insert(0,sys.argv[1]);import surface_offset_command_probe as p;p.validate(json.loads(sys.argv[2]))',folder,json.dumps(request()['operations'][0])],check=True,capture_output=True)
    def test_invalid_recipes_cannot_launch(self):
        q=request();op=q['operations'][0]
        for patch in [dict(op='other'),dict(id='../escape'),dict(case=[]),dict(case='unknown'),dict(actions=[{'curve':True},1.]),dict(actions=[{'curve':1},True]),dict(actions=[{'curve':1},'_Delete']),dict(action_prompts=[['Command'],['Distance']]),dict(extra=True)]:
            with self.subTest(patch=patch),self.assertRaises(ValueError):validate(dict(op,**patch))
        with mock.patch.dict(os.environ,dict(DISPLAY=':301',VIBOCEROS_ORACLE_HEADLESS=':301'),clear=True),mock.patch('tools.rhino_oracle.client._run_logged')as launch:
            for scheme,body in [(None,q),('VibocerosOracleSurfaceOffsetCommandUnit',dict(q,iterations=2)),('VibocerosOracleSurfaceOffsetCommandUnit',dict(q,operations=[op,op])),('VibocerosOracleSurfaceOffsetCommandUnit',dict(q,operations=[dict(op,op='line')]))]:
                # Include a valid command recipe to exercise the dedicated-job
                # guard for a mixed request, before any process can be started.
                if body['operations'][0].get('op')=='line':body=dict(body,operations=[op]+body['operations'])
                with self.assertRaises((ValueError,OracleProtocolError)):OracleClient(launcher=Path('/bin/true'),settings_scheme=scheme).run_rhino(body,1)
            launch.assert_not_called()
