"""Public Arc input receipts, preserved diagnostics and bounded staging."""
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path
from unittest import TestCase,mock
from .arc_tangent_probe import validate
from .client import OracleClient,OracleProtocolError
from .structural_sdk import stage_helpers
BASE=Path(__file__).parent

class ArcTangentTests(TestCase):
    def test_native_geometry_history_and_independent_input_receipts(self):
        total=clicks=0
        for profile,count in [('basic',7),('modes',10),('spatial',5),('memory',3),('curved',5),('curved_variants',7),('tolerance',2),('pair_types',1)]:
            q=json.loads((BASE/('fixtures/arc_tangent_'+profile+'.json')).read_text());n=json.loads((BASE/('observations/arc_tangent_'+profile+'_rhino_20261017.json')).read_text())
            self.assertEqual(len(q['operations']),count);self.assertEqual(len(n['results']),count)
            for op,row in zip(q['operations'],n['results']):
                validate(op);self.assertEqual(op['id'],row['id']);v=row['value'];t=v['input_trace']
                self.assertEqual(t['errors'],[]);self.assertEqual(t['completed'],len(op['actions']));self.assertNotIn('Unknown command',v['history'])
                self.assertEqual(v['before']['outputs'],v['undo']['outputs']);self.assertEqual(v['after_script']['outputs'],v['redo']['outputs'])
                self.assertEqual([e['result']for e in v['events']if e['name']=='Arc'],['Success'if v['after_script']['outputs']else'Cancel'])
                for stage in ['before','after','after_script','undo','redo']:self.assertTrue(v[stage]['guide_unchanged'])
                for c in t.get('clicks',[]):
                    self.assertTrue(c['motion_acknowledged']and c['click_acknowledged']);self.assertEqual(c['aim'],op['actions'][c['action']]['pick']);self.assertEqual(len(c['projected_point']),3);clicks+=1
                total+=1
        self.assertEqual(total,40);self.assertEqual(clicks,89)

    def test_qualified_subsets_and_numeric_diagnostics_preserve_raw_observations(self):
        for folder,key,suffix in [('fixtures','operations',''),('observations','results','_rhino_20261017')]:
            def rows(profile):return json.loads((BASE/folder/('arc_tangent_'+profile+suffix+'.json')).read_text())[key]
            for source,target,indices in [('initial','basic',[2,4,5,6,7,8,9]),('spatial_diagnostics','spatial',[0,1,2,3,6]),('curved_diagnostics','curved',[0,1,2,3,6]),('pair_types_diagnostics','pair_types',[1])]:
                raw=rows(source);self.assertEqual(rows(target),[raw[i]for i in indices])
            numeric=[]
            for source,indices in [('initial',[0,1,3]),('curved_diagnostics',[4,5]),('pair_types_diagnostics',[0,2])]:
                raw=rows(source);numeric.extend(raw[i]for i in indices)
            self.assertEqual(rows('numeric_diagnostics'),numeric)

    def test_bounds_private_capture_and_standalone_dependencies(self):
        q=json.loads((BASE/'fixtures/arc_tangent_basic.json').read_text());op=q['operations'][0]
        for field,value in [('actions',[]),('actions',['Tangent\nDelete']),('actions',[dict(curve=0)]),('actions',[dict(curve=True,pick=[0,0,0])]),('actions',[dict(curve=0,pick=[0,0,float('inf')])]),('live_getters',False)]:
            with self.assertRaises(ValueError):validate(dict(op,**{field:value}))
        with tempfile.TemporaryDirectory()as directory:
            job=Path(directory);stage_helpers(q,BASE,job)
            subprocess.run([sys.executable,'-I','-c','import sys,json;sys.path.insert(0,sys.argv[1]);import arc_tangent_probe;arc_tangent_probe.validate(json.loads(sys.argv[2]))',str(job),json.dumps(op)],check=True,capture_output=True)
        with mock.patch.dict(os.environ,dict(DISPLAY=':301',VIBOCEROS_ORACLE_HEADLESS=':301'),clear=True),mock.patch('tools.rhino_oracle.client._run_logged')as launch:
            for request in [dict(q,iterations=True),dict(q,iterations=2),dict(q,operations=[op,op]),dict(q,operations=[op,dict(op='line',id='foreign')])]:
                with self.assertRaises((ValueError,OracleProtocolError)):OracleClient(launcher=Path('/bin/true'),settings_scheme='VibocerosOracleArcTangentUnit').run_rhino(request,1)
            launch.assert_not_called()

    def test_native_pipe_regression_confirms_four_annular_cap_faces(self):
        n=json.loads((BASE/'observations/arc_tangent_pipe_regression_rhino_20261017.json').read_text())
        self.assertEqual(len(n['results']),2)
        for row in n['results']:
            v=row['value'];self.assertEqual(len(v['pieces']),1);piece=v['pieces'][0]
            self.assertTrue(piece['closed']and piece['valid']);self.assertEqual(piece['faces'],4)
            self.assertAlmostEqual(piece['volume'],5.921762640653614,places=6)
        self.assertTrue(n['results'][1]['value']['succeeded'])
