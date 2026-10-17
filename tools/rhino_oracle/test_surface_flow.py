"""Strict authored input validation and completed public native command captures."""
import copy
import json
import math
from pathlib import Path
from unittest import TestCase
from .surface_flow_probe import validate
from .structural_sdk import PROBES
BASE=Path(__file__).parent

class SurfaceFlowTests(TestCase):
    def read(self,path):return json.loads((BASE/path).read_text())
    def test_authored_recipes_and_bounded_numeric_inputs(self):
        for suffix in ['', '_command', '_faces', '_picks', '_groups']:
            for op in self.read('fixtures/surface_flow'+suffix+'.json')['operations']:validate(op)
        seed=self.read('fixtures/surface_flow.json')['operations'][0]
        for key,value in [('id','bad\nPICK'),('points',[]),('points',[[0,0,0]]*2049),('points',[[0,True,0]]),('points',[[0,float('nan'),0]]),('options',{'copy':1}),('options',{'unknown':True}),('normal',[0,0,0]),('normal',[0,0,float('inf')]),('selected',[-1]),('selected',[1,1]),('selected',[True]),('selected',[1]),('mouse',True),('edit',True),('peer',1),('base_trim',[[.8,.2],[0,1]])]:
            op=copy.deepcopy(seed);op[key]=value
            with self.assertRaises(ValueError,msg=key):validate(op)
        for role in ['base','target']:
            op=copy.deepcopy(seed);op[role]['control_points'][0]['point'][0]=float('nan')
            with self.assertRaises(ValueError):validate(op)
        self.assertEqual(PROBES['surface_flow'][0],'surface_flow_probe')
        self.assertIn('snap_environment',PROBES['surface_flow'][1])

    def test_native_commands_have_one_successful_event_and_real_undo_redo(self):
        total=0
        for suffix,count in [('_command',68),('_picks',16),('_faces',14),('_groups',8)]:
            q=self.read('fixtures/surface_flow'+suffix+'.json')
            n=self.read('observations/surface_flow'+suffix+'_rhino_20261017.json')
            self.assertEqual(len(q['operations']),count);self.assertEqual(len(n['results']),count)
            for op,row in zip(q['operations'],n['results']):
                self.assertEqual(op['id'],row['id']);v=row['value'];self.assertTrue(v['valid']);c=v['command']
                self.assertTrue(all(math.isfinite(x)for p in v['sdk_points']for x in p))
                for field in ['command_returned','undo_command_returned','redo_command_returned']:self.assertIs(c[field],True)
                self.assertEqual([e['result']for e in c['events']if e['name']=='FlowAlongSrf'],['Success'])
                for text in ['_Cancel','Unknown command','Invalid']:self.assertNotIn(text,c['history'])
                for state in ['before','after_script','undo','redo']:self.assertIs(c[state]['guide_unchanged'],True)
                if op.get('mouse'):self.assertEqual(c['interaction']['completed'],2)
                total+=1
        self.assertEqual(total,106)
