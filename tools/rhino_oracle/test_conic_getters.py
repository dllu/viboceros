"""Conic temporary getters: bounded sources, owned input and native evidence."""
import copy,json,os,tempfile
from pathlib import Path
from unittest import TestCase,mock
from .conic_probe import validate
from .conic_getter_picker import ConicGetterPicker
from .client import OracleClient,OracleProtocolError
BASE=Path(__file__).parent
class ConicGetterTests(TestCase):
    def request(self):return json.loads((BASE/'fixtures/conic_getters.json').read_text())
    def test_authored_sources_and_pointer_inputs_are_typed_before_native_allocation(self):
        q=self.request();self.assertEqual(len(q['operations']),28)
        for op in q['operations']:validate(op)
        surface=next(o for o in q['operations']if o['id']=='edge-start-tangent')
        for key,value in [('degree_u',True),('control_point_count_u',65),('knots_u',[0,0,1]),('control_points',[])]:
            op=copy.deepcopy(surface);op['curves'][0][key]=value
            with self.assertRaises(ValueError):validate(op)
        for action in [{'curve':0,'edge':True,'pick':[2,0,0]},{'curve':0,'edge':64,'pick':[2,0,0]},{'curve':0,'edge':0,'pick':[float('nan'),0,0]},{'curve':0},{'hover':[True,0,0]},{'hover':[0,0,0],'curve':0}]:
            op=copy.deepcopy(surface);op['actions'][1]=action
            with self.assertRaises(ValueError):validate(op)
        op=copy.deepcopy(q['operations'][0]);op['curves']=[dict(type='polyline',points=[[0,0,0],[10,0,0]])]
        with self.assertRaises(ValueError):validate(op)
    def test_only_requested_owned_motion_and_clicks_are_delivered(self):
        q=self.request();p=ConicGetterPicker(q)
        hover=next(iter(p.cases));name=p.cases[hover]['id']
        with tempfile.TemporaryDirectory()as directory,mock.patch('tools.rhino_oracle.subcurve_direction_input.subprocess.run')as send:
            p.job=Path(directory);self.assertFalse(p.send_input(hover,'100','200','owned'));argv=send.call_args.args[0]
            self.assertIn('owned',argv);self.assertIn('mousemove',argv);self.assertNotIn('click',argv);self.assertNotIn('type',argv);self.assertNotIn('key',argv)
            ready=p.job/('subcurve-direction-ready-'+name+'.json');ready.write_text(json.dumps('foreign'))
            with self.assertRaises(OracleProtocolError):p.send_input(hover,'100','200','owned')
            ready.write_text(json.dumps(hover));self.assertTrue(p.send_input(hover,'100','200','owned'))
            edge=next(iter(p.edges));self.assertTrue(p.send_input(edge,'100','200','owned'));self.assertIn('click',send.call_args.args[0]);self.assertIn('owned',send.call_args.args[0])
            with self.assertRaises(OracleProtocolError):p.record_diagnostics({})
            p.seen=set(p.cases)|p.edges;p.record_diagnostics({})
            with self.assertRaises(OracleProtocolError):p.send_input('foreign','100','200','owned')
    def test_client_requires_private_dedicated_single_iteration_pointer_jobs(self):
        q=self.request()
        with mock.patch.dict(os.environ,dict(DISPLAY=':301',VIBOCEROS_ORACLE_HEADLESS=':301'),clear=True),mock.patch('tools.rhino_oracle.client._run_logged')as launch:
            for index,request in enumerate([q,dict(q,iterations=2),dict(q,operations=q['operations']+[dict(op='line',id='foreign')]),dict(q,operations=q['operations']*2)]):
                with self.assertRaises((ValueError,OracleProtocolError)):OracleClient(launcher=Path('/bin/true'),settings_scheme=None if index==0 else "VibocerosOracleConicUnit").run_rhino(request,1)
            launch.assert_not_called()
    def test_native_getters_preserve_sources_and_complete_all_physical_inputs(self):
        q=self.request();n=json.loads((BASE/'observations/conic_getters_rhino_20261017.json').read_text())
        self.assertEqual(len(n['results']),28)
        for op,row in zip(q['operations'],n['results']):
            self.assertEqual(op['id'],row['id']);v=row['value'];self.assertTrue(v['command_returned']);self.assertEqual(v['after'],v['after_script']);self.assertEqual([e['result']for e in v['events']if e['name']=='Conic'],['Success'])
            self.assertTrue(v['undo_returned']);self.assertTrue(v['redo_returned']);self.assertEqual(v['before'],v['undo']);self.assertEqual(v['after'],v['redo']);self.assertEqual(len(v['after']['outputs']),1)
            for stage in ['before','after','undo','redo']:self.assertTrue(v[stage]['guide_unchanged'])
            trace=v['input_trace'];self.assertEqual(trace['completed'],trace['expected']);self.assertEqual(trace['expected'],len(op['actions']));self.assertEqual(trace['errors'],[])
            self.assertEqual(len(trace['motion']),sum(isinstance(a,dict)and'hover'in a for a in op['actions']));self.assertEqual(len(trace['edge_picks']),sum(isinstance(a,dict)and'edge'in a for a in op['actions']))
            self.assertTrue(all(m['acknowledged']for m in trace['motion']+trace['edge_picks']));self.assertIn('OnEndMouseMove',trace['motion_acknowledgement'])
            for token in ['_Cancel','Unknown command','Invalid']:self.assertNotIn(token,v['history'])
