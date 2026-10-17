"""Ellipse capture bounds, native getter receipts and separate failure controls."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
from unittest import TestCase, mock

from .client import OracleClient, OracleProtocolError
from .ellipse_probe import validate
from .structural_sdk import stage_helpers
BASE=Path(__file__).parent


class EllipseTests(TestCase):
    def test_native_modes_geometry_markers_and_actual_history_have_complete_receipts(self):
        total=0
        for profile,count in [('command',16),('directions',18),('final',11),('locks',5),('planes',5)]:
            q=json.loads((BASE/('fixtures/ellipse_'+profile+'.json')).read_text())
            n=json.loads((BASE/('observations/ellipse_'+profile+'_rhino_20261017.json')).read_text())
            self.assertEqual(len(q['operations']),count)
            self.assertEqual(len(n['results']),count)
            for op,row in zip(q['operations'],n['results']):
                validate(op);self.assertEqual(op['id'],row['id'])
                v=row['value']
                self.assertEqual([e['result']for e in v['events']if e['name']=='Ellipse'],['Success'])
                self.assertEqual(v['before'],v['undo']);self.assertEqual(v['after'],v['after_script']);self.assertEqual(v['after'],v['redo'])
                self.assertTrue(v['undo_returned']);self.assertTrue(v['redo_returned'])
                self.assertTrue(all(v[s]['guide_unchanged']for s in ['before','after','undo','redo']))
                for field,actions in [('input_trace',op['actions'])]+([('setup_input_trace',op['setup'])]if'setup'in op else[]):
                    trace=v[field];self.assertEqual(trace['errors'],[]);self.assertEqual(trace['completed'],len(actions));self.assertEqual(trace['expected'],len(actions));self.assertTrue(all(trace['prompts']))
                outputs=v['after']['outputs'];curves=[o for o in outputs if'curve'in o];points=[o for o in outputs if'point'in o]
                self.assertEqual(len(curves),1);self.assertIn(len(points),(0,2));self.assertTrue(all(not o['selected']for o in outputs))
                c=curves[0]['curve'];self.assertEqual(c['degree'],2);self.assertEqual(len(c['control_points']),9)
                if points:self.assertTrue(all('point'in o for o in outputs[:2]))
                total+=1
        self.assertEqual(total,55)

    def test_rejections_and_uncontrolled_cursor_inputs_are_not_geometry_qualifications(self):
        q=json.loads((BASE/'fixtures/ellipse_rejections.json').read_text())
        n=json.loads((BASE/'observations/ellipse_rejections_rhino_20261017.json').read_text())
        self.assertEqual(len(q['operations']),8)
        results=[]
        for op,row in zip(q['operations'],n['results']):
            validate(op);self.assertEqual(op['id'],row['id']);v=row['value'];results += [e['result']for e in v['events']if e['name']=='Ellipse']
            self.assertEqual(v['input_trace']['errors'],[]);self.assertEqual(v['input_trace']['completed'],len(op['actions']))
            self.assertTrue(all(v[s]['outputs']==[]for s in ['before','after','after_script','undo','redo']))
        self.assertEqual(results.count('Failure'),5);self.assertEqual(results.count('Cancel'),3)
        for profile,count in [('direction_diagnostics',2),('diagnostics',4)]:
            q=json.loads((BASE/('fixtures/ellipse_'+profile+'.json')).read_text());n=json.loads((BASE/('observations/ellipse_'+profile+'_rhino_20261017.json')).read_text())
            self.assertEqual(len(q['operations']),count)
            for op,row in zip(q['operations'],n['results']):validate(op);self.assertEqual(op['id'],row['id'])

    def test_bounded_recipes_private_capture_and_standalone_staging(self):
        q=json.loads((BASE/'fixtures/ellipse_command.json').read_text());op=q['operations'][0]
        for field,value in [('id','bad\ncommand'),('extra',True),('actions',[]),('actions',[True]),('actions',[10**500]),('actions',[[0,0,float('inf')]]),('actions',['_Delete']),('actions',[{'click':[0,0,0]}]),('actions',[{'relative':[0,0,True]}]),('actions',['Enter']*33),('live_getters',1),('construction_plane',[[0,0,0]]*2)]:
            with self.assertRaises(ValueError,msg=(field,value)):validate(dict(op,**{field:value}))
        with tempfile.TemporaryDirectory()as directory:
            job=Path(directory);stage_helpers(q,BASE,job)
            self.assertEqual({p.name for p in job.iterdir()},{'ellipse_probe.py','curve_primitive_probe.py','join_probe.py','merge_edges_probe.py','conic_getter_input.py','managed_mouse_callback.py','snap_environment.py'})
            subprocess.run([sys.executable,'-I','-c','import sys,json;sys.path.insert(0,sys.argv[1]);import ellipse_probe;ellipse_probe.validate(json.loads(sys.argv[2]))',str(job),json.dumps(op)],check=True,capture_output=True)
        with mock.patch.dict(os.environ,dict(DISPLAY=':301',VIBOCEROS_ORACLE_HEADLESS=':301'),clear=True),mock.patch('tools.rhino_oracle.client._run_logged')as launch:
            for i,request in enumerate([q,dict(q,iterations=2),dict(q,iterations=True),dict(q,operations=[op,op]),dict(q,operations=[op,dict(op='line',id='foreign')])]):
                with self.assertRaises((ValueError,OracleProtocolError)):
                    OracleClient(launcher=Path('/bin/true'),settings_scheme=None if i==0 else 'VibocerosOracleEllipseUnit').run_rhino(request,1)
            launch.assert_not_called()
