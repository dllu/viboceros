"""Conic getter input bounds, isolated worker staging and native history evidence."""
import copy,json,subprocess,sys,tempfile
from pathlib import Path
from unittest import TestCase
from .conic_probe import validate
from .structural_sdk import stage_helpers
BASE=Path(__file__).parent
class ConicTests(TestCase):
    def test_bounded_authored_inputs_and_standalone_worker_dependencies(self):
        q=json.loads((BASE/'fixtures/conic_command.json').read_text())
        for op in q['operations']:validate(op)
        seed=q['operations'][0]
        for key,value in [('id','bad\nPICK'),('actions',[]),('actions',['Tangent']*25),('actions',[True]),('actions',[[0,0,float('nan')]]),('actions',[float('inf')]),('actions',['Cancel']),('actions',[{'curve':True}]),('actions',[{'curve':0}]),('setup',['Unknown']),('curves',[{'type':'point','point':[0,0,0]}])]:
            op=copy.deepcopy(seed);op[key]=value
            with self.assertRaises(ValueError,msg=(key,value)):validate(op)
        with tempfile.TemporaryDirectory()as directory:
            job=Path(directory);stage_helpers(q,BASE,job)
            self.assertEqual({p.name for p in job.iterdir()},{'conic_probe.py','curve_primitive_probe.py','geometry_validity_probe.py','join_probe.py','merge_edges_probe.py'})
            subprocess.run([sys.executable,'-I','-c','import sys;sys.path.insert(0,sys.argv[1]);import conic_probe,curve_primitive_probe;conic_probe.validate(__import__("json").loads(sys.argv[2])["operations"][0])',str(job),json.dumps(q)],check=True,capture_output=True)
    def test_native_command_events_geometry_source_preservation_and_undo_redo(self):
        q=json.loads((BASE/'fixtures/conic_command.json').read_text());n=json.loads((BASE/'observations/conic_command_rhino_20261017.json').read_text())
        self.assertEqual(len(q['operations']),42);self.assertEqual(len(n['results']),42)
        for op,row in zip(q['operations'],n['results']):
            self.assertEqual(op['id'],row['id']);self.assertNotIn('cancel_tail',op);v=row['value'];self.assertTrue(v['command_returned']);self.assertEqual([e['result']for e in v['events']if e['name']=='Conic'],['Success'])
            for token in ['_Cancel','Unknown command','Invalid']:self.assertNotIn(token,v['history'])
            self.assertEqual(v['after'],v['after_script']);self.assertTrue(v['undo_returned']);self.assertTrue(v['redo_returned']);self.assertEqual(v['before'],v['undo']);self.assertEqual(v['after'],v['redo']);self.assertEqual(len(v['after']['outputs']),1)
            for stage in ['before','after','undo','redo']:self.assertTrue(v[stage]['guide_unchanged'])

    def test_default_length_query_control_retains_the_original_measurement(self):
        n=json.loads((BASE/'observations/conic_command_default_length_rhino_20261017.json').read_text())
        row=n['results'][0];self.assertEqual(row['id'],'conic3-apex-past-end')
        # x(t)=30t-20t² turns at t=3/4: 11.25+(11.25-10)=12.5.
        self.assertGreater(abs(row['value']['after']['outputs'][0]['length']-12.5),1e-6)
        precise=json.loads((BASE/'observations/conic_command_rhino_20261017.json').read_text())
        row=next(r for r in precise['results']if r['id']=='conic3-apex-past-end')
        self.assertLess(abs(row['value']['after']['outputs'][0]['length']-12.5),1e-6)
