"""Native Extension input receipts, full geometry, history and capture bounds."""
import json,os,subprocess,sys,tempfile
from pathlib import Path
from unittest import TestCase,mock
from .arc_extension_probe import validate
from .client import OracleClient,OracleProtocolError
from .structural_sdk import stage_helpers
BASE=Path(__file__).parent
class ArcExtensionTests(TestCase):
    def test_native_public_workflows_and_input_calibration(self):
        total=success=0
        for name,count in [('basic',4),('center_numeric',2),('center_cursor',2),('sources',8),('selection',2),('end_choice',1),('relative',1)]:
            q=json.loads((BASE/f'fixtures/arc_extension_{name}.json').read_text());n=json.loads((BASE/f'observations/arc_extension_{name}_rhino_20261017.json').read_text())
            self.assertEqual(len(q['operations']),count);self.assertEqual(len(n['results']),count)
            for op,row in zip(q['operations'],n['results']):
                validate(op);self.assertEqual(op['id'],row['id']);v=row['value'];t=v['input_trace']
                self.assertEqual(t['errors'],[]);self.assertEqual(t['completed'],len(op['actions']));self.assertNotIn('Unknown command',v['history'])
                for stage in ['before','after','after_script','undo','redo']:self.assertTrue(v[stage]['guide_unchanged'])
                self.assertEqual(v['before']['outputs'],v['undo']['outputs']);self.assertEqual(v['after_script']['outputs'],v['redo']['outputs'])
                output=bool(v['after_script']['outputs']);success+=output;total+=1
                self.assertEqual([e['result']for e in v['events']if e['name']=='Arc'],['Success'if output else'Cancel'])
                for c in t['clicks']:
                    self.assertTrue(c['motion_acknowledged']and c['click_acknowledged']);self.assertEqual(len(c['projected_point']),3)
                    if 'curve'not in c:self.assertEqual(len(c['initial_projected_point']),3)
        self.assertEqual((total,success),(20,18))
    def test_qualified_rows_are_unchanged_and_native_length_diagnostic_is_preserved(self):
        for folder,key,suffix in [('fixtures','operations',''),('observations','results','_rhino_20261017')]:
            def rows(profile):return json.loads((BASE/folder/f'arc_extension_{profile}{suffix}.json').read_text())[key]
            for source,target,indices in [('initial','basic',[0,1,3,5]),('center','center_numeric',[0,3]),('diagnostics','selection',[0,2])]:self.assertEqual(rows(target),[rows(source)[i]for i in indices])
        n=json.loads((BASE/'observations/arc_extension_initial_rhino_20261017.json').read_text())
        self.assertIn('Select curves to measure',n['results'][4]['value']['input_trace']['prompts'][-1])
        self.assertEqual(n['results'][4]['value']['input_trace']['errors'],['owned Arc input timeout'])
        d=json.loads((BASE/'observations/arc_extension_diagnostics_rhino_20261017.json').read_text())['results'][1]['value']
        self.assertEqual(d['after_script']['outputs'],[])
        self.assertEqual([e['result']for e in d['events']if e['name']=='Arc'],['Failure'])
    def test_private_bounds_and_standalone_helper_dependencies(self):
        q=json.loads((BASE/'fixtures/arc_extension_basic.json').read_text());op=q['operations'][0]
        for actions in [[],['Extension\nDelete'],['Extension',dict(curve=0)],['Extension',dict(click=[0,0,0])],['Extension',dict(curve=True,pick=[0,0,0])]]:
            with self.assertRaises(ValueError):validate(dict(op,actions=actions))
        with tempfile.TemporaryDirectory()as directory:
            stage_helpers(q,BASE,Path(directory))
            subprocess.run([sys.executable,'-I','-c','import sys,json;sys.path.insert(0,sys.argv[1]);import arc_extension_probe;arc_extension_probe.validate(json.loads(sys.argv[2]))',directory,json.dumps(op)],check=True,capture_output=True)
        with mock.patch.dict(os.environ,dict(DISPLAY=':308',VIBOCEROS_ORACLE_HEADLESS=':308'),clear=True),mock.patch('tools.rhino_oracle.client._run_logged')as launch:
            for request in [dict(q,iterations=True),dict(q,iterations=2),dict(q,operations=[op,op]),dict(q,operations=[op,dict(op='line',id='foreign')])]:
                with self.assertRaises((ValueError,OracleProtocolError)):OracleClient(launcher=Path('/bin/true'),settings_scheme='VibocerosOracleArcExtensionUnit').run_rhino(request,1)
            launch.assert_not_called()
