"""Bounded Circle AroundCurve native getter receipts and source/history evidence."""
import json,os,subprocess,sys,tempfile
from pathlib import Path
from unittest import TestCase,mock
from .circle_around_probe import validate
from .client import OracleClient,OracleProtocolError
from .structural_sdk import stage_helpers
BASE=Path(__file__).parent
class CircleAroundTests(TestCase):
    def test_native_guides_sizes_relative_input_preferences_edges_and_history(self):
        total=edges=0
        for profile,count in [('command',10),('extended',22),('numeric',9),('memory',2)]:
            q=json.loads((BASE/('fixtures/circle_around_'+profile+'.json')).read_text());n=json.loads((BASE/('observations/circle_around_'+profile+'_rhino_20261017.json')).read_text());self.assertEqual(len(q['operations']),count);self.assertEqual(len(n['results']),count)
            for op,row in zip(q['operations'],n['results']):
                validate(op);self.assertEqual(op['id'],row['id']);v=row['value'];t=v['input_trace'];self.assertEqual([e['result']for e in v['events']if e['name']=='Circle'],['Success']);self.assertEqual(t['errors'],[]);self.assertEqual(t['completed'],t['expected']);self.assertEqual(t['completed'],len(op['actions']));self.assertNotIn('Unknown command',v['history'])
                self.assertEqual(v['before']['outputs'],v['undo']['outputs']);self.assertEqual(v['after']['outputs'],v['after_script']['outputs']);self.assertEqual(v['after']['outputs'],v['redo']['outputs']);self.assertTrue(v['undo_returned']);self.assertTrue(v['redo_returned'])
                self.assertEqual(len(v['after']['outputs']),1);self.assertEqual(v['after']['outputs'][0]['curve']['degree'],2);self.assertFalse(v['after']['outputs'][0]['selected'])
                if 'setup'in op:self.assertEqual(v['setup_input_trace']['errors'],[]);self.assertEqual(v['setup_input_trace']['completed'],len(op['setup']))
                picks=[a for a in op['actions']if isinstance(a,dict)and'edge'in a]
                self.assertEqual(len(picks),len(t['edge_picks']))
                for pick,receipt in zip(picks,t['edge_picks']):self.assertTrue(receipt['acknowledged']);self.assertEqual(receipt['authored_edge'],pick['edge']);self.assertEqual(receipt['pick'],pick['pick']);edges+=1
                for stage in ['before','after','after_script','undo','redo']:
                    self.assertTrue(v[stage]['guide_unchanged']);self.assertEqual(v[stage]['guide_selected'],[False if picks else True if stage=='after'else op.get('preselect',False)])
                total+=1
        self.assertEqual((total,edges),(43,5))
    def test_native_cancellations_and_unsupported_orientation_are_separate(self):
        q=json.loads((BASE/'fixtures/circle_around_controls.json').read_text());n=json.loads((BASE/'observations/circle_around_controls_rhino_20261017.json').read_text());self.assertEqual(len(q['operations']),5)
        for op,row in zip(q['operations'],n['results']):
            validate(op);v=row['value'];self.assertEqual(v['input_trace']['errors'],[]);self.assertEqual(v['input_trace']['completed'],len(op['actions']));self.assertEqual([e['result']for e in v['events']if e['name']=='Circle'],['Cancel'])
            for stage in ['before','after','after_script','undo','redo']:self.assertEqual(v[stage]['outputs'],[]);self.assertTrue(v[stage]['guide_unchanged'])
            for stage in ['after_script','undo','redo']:self.assertEqual(v[stage]['guide_selected'],[op.get('preselect',False)])
        q=json.loads((BASE/'fixtures/circle_around_diagnostics.json').read_text());n=json.loads((BASE/'observations/circle_around_diagnostics_rhino_20261017.json').read_text());self.assertEqual(len(q['operations']),1);self.assertIn('Unknown command: _Orientation',n['results'][0]['value']['history']);self.assertTrue(n['results'][0]['value']['input_trace']['errors'])
    def test_bounds_private_capture_and_standalone_helper_dependencies(self):
        q=json.loads((BASE/'fixtures/circle_around_command.json').read_text());op=q['operations'][0]
        for field,value in [('id','bad\ncommand'),('actions',[]),('actions',[True]),('actions',[10**500]),('actions',['Radius=NaN']),('actions',['Area=1e999']),('actions',['Diameter=3 Delete']),('actions',['_Delete']),('actions',[dict(curve=True)]),('actions',[dict(curve=4)]),('curves',op['curves']*5),('live_getters',False),('live_getters',1),('preselect',1),('extra',True)]:
            with self.assertRaises(ValueError,msg=(field,value)):validate(dict(op,**{field:value}))
        with tempfile.TemporaryDirectory()as directory:
            job=Path(directory);stage_helpers(q,BASE,job);self.assertEqual({p.name for p in job.iterdir()},{'circle_around_probe.py','getter_recipe.py','curve_source_bounds.py','geometry_validity_probe.py','curve_primitive_probe.py','join_probe.py','merge_edges_probe.py','conic_getter_input.py','managed_mouse_callback.py','snap_environment.py'})
            subprocess.run([sys.executable,'-I','-c','import sys,json;sys.path.insert(0,sys.argv[1]);import circle_around_probe;circle_around_probe.validate(json.loads(sys.argv[2]))',str(job),json.dumps(op)],check=True,capture_output=True)
        with mock.patch.dict(os.environ,dict(DISPLAY=':301',VIBOCEROS_ORACLE_HEADLESS=':301'),clear=True),mock.patch('tools.rhino_oracle.client._run_logged')as launch:
            for i,request in enumerate([q,dict(q,iterations=2),dict(q,iterations=True),dict(q,operations=[op,op]),dict(q,operations=[op,dict(op='line',id='foreign')])]):
                with self.assertRaises((ValueError,OracleProtocolError)):OracleClient(launcher=Path('/bin/true'),settings_scheme=None if i==0 else 'VibocerosOracleCircleAroundUnit').run_rhino(request,1)
            launch.assert_not_called()
