"""Curve-to-edge input bounds, public getter delivery, and frozen evidence."""
import gzip,hashlib,json,os,subprocess,sys,tempfile
from pathlib import Path
from unittest import TestCase,mock
from .client import OracleClient,OracleProtocolError
from .generate_orient_curve_edge import request,option_request,spatial_request,parallel_request,threshold_request,cutoff_request,boundary_request,angle_request,subcurve_request
from .structural_sdk import stage_helpers,validate_operations
HERE=Path(__file__).resolve().parent

class OrientCurveEdgeTests(TestCase):
    def test_owned_subcurve_hover_requires_a_matching_public_acknowledgement(self):
        from .arc_point_picker import ArcPointPicker
        q=subcurve_request();op=next(o for o in q['operations']if o['id']=='subcurve_locked_length');q=dict(q,operations=[dict(op,actions=[dict(click=a['pick'])if isinstance(a,dict)and'curve'in a else a for a in op['actions']])]);p=ArcPointPicker(q)
        marker=next(iter(p.hovers));name=p.hovers[marker]['id']
        with tempfile.TemporaryDirectory()as folder,mock.patch('tools.rhino_oracle.subcurve_direction_input.subprocess.run')as send:
            p.job=Path(folder);self.assertFalse(p.send_input(marker,'100','200','owned'));args=send.call_args.args[0];self.assertIn('owned',args);self.assertIn('mousemove',args);self.assertNotIn('click',args);self.assertNotIn('key',args)
            ready=p.job/('subcurve-direction-ready-'+name+'.json');ready.write_text(json.dumps('foreign'))
            with self.assertRaises(OracleProtocolError):p.send_input(marker,'100','200','owned')
            ready.write_text(json.dumps(marker));self.assertTrue(p.send_input(marker,'100','200','owned'))
            with self.assertRaises(OracleProtocolError):p.send_input('foreign','100','200','owned')
            with self.assertRaises(OracleProtocolError):p.record_diagnostics({})
            p.seen=set(p.cases);p.record_diagnostics({})
    def test_reproducible_captures_and_source_closure(self):
        for label,q,count in [('orient_curve_edge_subcurve',subcurve_request(),33),('orient_curve_edge',request(),43),('orient_curve_edge_options',option_request(),6),('orient_curve_edge_spatial',spatial_request(),39),('orient_curve_edge_parallel',parallel_request(),26),('orient_curve_edge_threshold',threshold_request(),19),('orient_curve_edge_cutoff',cutoff_request(),20),('orient_curve_edge_boundary',boundary_request(),24),('orient_curve_edge_angle',angle_request(),12)]:
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
                if 'actual_tolerance'in v:self.assertEqual(v['actual_tolerance']['angular'],op.get('angle_tolerance',q['tolerance']['angular']))
        q=request()
        with tempfile.TemporaryDirectory()as folder:
            stage_helpers(q,HERE,Path(folder))
            for name in ['orient_curve_edge_probe.py','conic_getter_input.py','managed_mouse_callback.py','join_probe.py','merge_edges_probe.py','snap_environment.py','number_token.py']:self.assertEqual((Path(folder)/name).read_bytes(),(HERE/name).read_bytes())
            subprocess.run([sys.executable,'-I','-c','import sys,json;sys.path.insert(0,sys.argv[1]);import orient_curve_edge_probe;p=json.loads(sys.argv[2]);orient_curve_edge_probe.validate(p)',folder,json.dumps(q['operations'][0])],check=True,capture_output=True)
    def test_invalid_recipes_fail_before_launch(self):
        q=request();op=q['operations'][0]
        for patch in [dict(extra=True),dict(angle_tolerance=0),dict(angle_tolerance=True),dict(angle_tolerance=float('nan')),dict(angle_tolerance=float('inf')),dict(id='../escape'),dict(id='a\nb'),dict(id='x'*81),dict(points=[]),dict(corners=[]),dict(grouped=1),dict(kind='Unknown'),dict(reversed=1),dict(cplane=[[0,0,float('nan')]]*3),dict(actions=['_Delete']),dict(actions=[{'curve':True}]),dict(actions=[{'curve':2}]),dict(actions=[{'curve':1,'edge':4,'pick':[0,0,0]}]),dict(actions=[[True,0,0]]),dict(actions=[[0,0,1e9]]),dict(action_prompts=[['Command']]*6)]:
            with self.subTest(patch=patch),self.assertRaises(ValueError):validate_operations(dict(operations=[dict(op,**patch)]))
        with mock.patch.dict(os.environ,dict(DISPLAY=':301',VIBOCEROS_ORACLE_HEADLESS=':301'),clear=True),mock.patch('tools.rhino_oracle.client._run_logged')as launch:
            for scheme,req in [(None,q),('VibocerosOracleOrientEdgeUnit',dict(q,iterations=2)),('VibocerosOracleOrientEdgeUnit',dict(q,operations=[op,op])),('VibocerosOracleOrientEdgeUnit',dict(q,operations=q['operations']*2))]:
                with self.assertRaises((ValueError,OracleProtocolError)):OracleClient(launcher=Path('/bin/true'),settings_scheme=scheme).run_rhino(req,1)
            launch.assert_not_called()
