"""Tilted Arc public input receipts, history and bounded SDK staging."""
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path
from unittest import TestCase,mock
from .arc_command_probe import validate
from .client import OracleClient,OracleProtocolError
from .structural_sdk import stage_helpers
BASE=Path(__file__).parent

class ArcCommandTests(TestCase):
    def test_native_tilted_getters_full_history_and_immutable_qualified_subsets(self):
        total=success=cancel=0
        for profile,count in [('initial',2),('modes',11),('endings',2),('spatial',6),('cursor',12)]:
            q=json.loads((BASE/f'fixtures/arc_tilted_{profile}.json').read_text());n=json.loads((BASE/f'observations/arc_tilted_{profile}_rhino_20261017.json').read_text())
            self.assertEqual(len(q['operations']),count);self.assertEqual(len(n['results']),count)
            for op,row in zip(q['operations'],n['results']):
                validate(op);self.assertEqual(op['id'],row['id']);v=row['value'];t=v['input_trace']
                self.assertEqual(t['errors'],[]);self.assertEqual(t['completed'],len(op['actions']));self.assertNotIn('Unknown command',v['history'])
                if 'setup'in op:self.assertEqual(v['setup_input_trace']['errors'],[]);self.assertEqual(v['setup_input_trace']['completed'],len(op['setup']))
                self.assertEqual(v['before']['outputs'],v['undo']['outputs']);self.assertEqual(v['after_script']['outputs'],v['redo']['outputs'])
                has_output=bool(v['after_script']['outputs']);success+=has_output;cancel+=not has_output
                self.assertEqual([e['result']for e in v['events']if e['name']=='Arc'],['Success'if has_output else'Cancel']);total+=1
        self.assertEqual((total,success,cancel),(33,30,3))
        for folder,key,suffix in [('fixtures','operations',''),('observations','results','_rhino_20261017')]:
            def rows(profile):return json.loads((BASE/folder/f'arc_tilted_{profile}{suffix}.json').read_text())[key]
            for raw,target,indices in [('diagnostics','modes',[2,3,4,5,6,8,9,10,11,13,14]),('endings_diagnostics','endings',[4,5]),('paths','spatial',[8,9,10,11,12,15])]:
                source=rows(raw);self.assertEqual(rows(target),[source[i]for i in indices])

    def test_endpoint_pointer_receipts_do_not_rewrite_output_geometry(self):
        q=json.loads((BASE/'fixtures/arc_tilted_paths.json').read_text());n=json.loads((BASE/'observations/arc_tilted_paths_rhino_20261017.json').read_text())
        for op,row in zip(q['operations'][:8],n['results'][:8]):
            validate(op);v=row['value'];self.assertEqual(v['input_trace']['errors'],[])
            c=v['input_trace']['clicks'][0];self.assertTrue(c['motion_acknowledged']and c['click_acknowledged'])
            self.assertEqual(c['aim'],op['actions'][c['action']]['click']);self.assertEqual(len(c['projected_point']),3)
            self.assertTrue(v['input_trace']['pointer_path']);self.assertEqual([e['result']for e in v['events']if e['name']=='Arc'],['Success'])

    def test_bounds_private_capture_and_standalone_helpers(self):
        q=json.loads((BASE/'fixtures/arc_tilted_initial.json').read_text());op=q['operations'][0]
        for field,value in [('actions',[]),('actions',['Tilted\nDelete']),('actions',[dict(click=[0,0,0])]),('actions',[dict(click=[0,0,float('inf')])]),('live_getters',False),('curves',[dict(type='line',start=[0,0,0],end=[1,0,0])])]:
            with self.assertRaises(ValueError):validate(dict(op,**{field:value}))
        with tempfile.TemporaryDirectory()as directory:
            stage_helpers(q,BASE,Path(directory))
            subprocess.run([sys.executable,'-I','-c','import sys,json;sys.path.insert(0,sys.argv[1]);import arc_command_probe;arc_command_probe.validate(json.loads(sys.argv[2]))',directory,json.dumps(op)],check=True,capture_output=True)
        with mock.patch.dict(os.environ,dict(DISPLAY=':302',VIBOCEROS_ORACLE_HEADLESS=':302'),clear=True),mock.patch('tools.rhino_oracle.client._run_logged')as launch:
            for request in [dict(q,iterations=True),dict(q,iterations=2),dict(q,operations=[op,op]),dict(q,operations=[op,dict(op='line',id='foreign')])]:
                with self.assertRaises((ValueError,OracleProtocolError)):OracleClient(launcher=Path('/bin/true'),settings_scheme='VibocerosOracleArcCommandUnit').run_rhino(request,1)
            launch.assert_not_called()
