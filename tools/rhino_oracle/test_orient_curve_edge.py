"""Curve-to-edge input bounds, public getter delivery, and frozen evidence."""
import copy,gzip,hashlib,json,os,subprocess,sys,tempfile
from pathlib import Path
from unittest import TestCase,mock
from .client import OracleClient,OracleProtocolError
from .generate_orient_curve_edge import request,option_request
from .structural_sdk import stage_helpers,validate_operations
HERE=Path(__file__).resolve().parent

class OrientCurveEdgeTests(TestCase):
    def test_reproducible_captures_and_source_closure(self):
        for label,q,count in [('orient_curve_edge',request(),43),('orient_curve_edge_options',option_request(),6)]:
            path=HERE/('fixtures/'+label+'.json');self.assertEqual(q,json.loads(path.read_text()));r=json.loads((HERE/('observations/'+label+'.json')).read_text());self.assertEqual(len(r['results']),count)
            self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(),r['provenance']['request_sha256'])
            for name,sha in r['provenance']['sources'].items():
                data=(HERE/name).read_bytes()
                if hashlib.sha256(data).hexdigest()!=sha:data=gzip.decompress((HERE/'observations/source_snapshots'/(sha+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(data).hexdigest(),sha,name)
            for op,row in zip(q['operations'],r['results']):
                self.assertEqual(op['id'],row['id']);self.assertNotIn('error',row);v=row['value'];t=v['input_trace'];self.assertEqual(t['errors'],[]);self.assertEqual(t['completed'],len(op['actions']));self.assertEqual(t['expected'],t['completed']);self.assertNotIn('Unknown command',v['history'])
                for sent in t['sent']:self.assertTrue(any(sent['prompt'].startswith(p)for p in op['action_prompts'][sent['action']]))
                for click in t.get('clicks',[]):self.assertTrue(click['motion_acknowledged']);self.assertTrue(click['click_acknowledged'])
                self.assertEqual(v['after_script'],v['redo']);self.assertEqual(v['before'],v['undo'])
                for phase in ['after_script','undo','redo']:
                    targets=[o for o in v[phase]if o['role']=='target'];self.assertEqual(targets,[o for o in v['before']if o['role']=='target'])
                self.assertTrue(all(not o['selected']for o in v['after_script']))
        q=request()
        with tempfile.TemporaryDirectory()as folder:
            stage_helpers(q,HERE,Path(folder))
            for name in ['orient_curve_edge_probe.py','conic_getter_input.py','managed_mouse_callback.py','join_probe.py','merge_edges_probe.py','snap_environment.py','number_token.py']:self.assertEqual((Path(folder)/name).read_bytes(),(HERE/name).read_bytes())
            subprocess.run([sys.executable,'-I','-c','import sys,json;sys.path.insert(0,sys.argv[1]);import orient_curve_edge_probe;p=json.loads(sys.argv[2]);orient_curve_edge_probe.validate(p)',folder,json.dumps(q['operations'][0])],check=True,capture_output=True)
    def test_invalid_recipes_fail_before_launch(self):
        q=request();op=q['operations'][0]
        for patch in [dict(extra=True),dict(id='../escape'),dict(id='a\nb'),dict(id='x'*81),dict(points=[]),dict(corners=[]),dict(grouped=1),dict(kind='Unknown'),dict(reversed=1),dict(cplane=[[0,0,float('nan')]]*3),dict(actions=['_Delete']),dict(actions=[{'curve':True}]),dict(actions=[{'curve':2}]),dict(actions=[{'curve':1,'edge':4,'pick':[0,0,0]}]),dict(actions=[[True,0,0]]),dict(actions=[[0,0,1e9]]),dict(action_prompts=[['Command']]*6)]:
            with self.subTest(patch=patch),self.assertRaises(ValueError):validate_operations(dict(operations=[dict(op,**patch)]))
        with mock.patch.dict(os.environ,dict(DISPLAY=':301',VIBOCEROS_ORACLE_HEADLESS=':301'),clear=True),mock.patch('tools.rhino_oracle.client._run_logged')as launch:
            for scheme,req in [(None,q),('VibocerosOracleOrientEdgeUnit',dict(q,iterations=2)),('VibocerosOracleOrientEdgeUnit',dict(q,operations=[op,op])),('VibocerosOracleOrientEdgeUnit',dict(q,operations=q['operations']*2))]:
                with self.assertRaises((ValueError,OracleProtocolError)):OracleClient(launcher=Path('/bin/true'),settings_scheme=scheme).run_rhino(req,1)
            launch.assert_not_called()
