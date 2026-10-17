"""Rectangle command completion, curve witnesses, and explicit native exceptions."""
import copy,json,math
from pathlib import Path
from unittest import TestCase
from .rectangle_probe import validate
from .structural_sdk import PROBES
BASE=Path(__file__).parent
class RectangleTests(TestCase):
    def read(self,path):return json.loads((BASE/path).read_text())
    def test_rounded_inputs_are_bounded_and_typed(self):
        q=self.read('fixtures/rectangle_rounded.json')
        for op in q['operations']:validate(op)
        seed=q['operations'][0]
        for key,value in [('id','bad\nPICK'),('actions',[]),('actions',['Rounded']*25),('actions',[True]),('actions',[[0,0,float('nan')]]),('actions',[float('inf')]),('actions',['Cancel']),('actions',['Radius='+('1'*100)]),('actions',['Rounded _Cancel']),('preselect',1),('setup',['Bad']),('construction_plane',[[0,0,0],[1,0,0],[0,1,float('inf')]])]:
            op=copy.deepcopy(seed);op[key]=value
            with self.assertRaises(ValueError,msg=key):validate(op)
        self.assertEqual(PROBES['rectangle'][0],'rectangle_probe')
        self.assertIn('merge_edges_probe',PROBES['rectangle'][1])
    def test_sixty_geometry_witnesses_and_two_native_no_output_controls(self):
        q=self.read('fixtures/rectangle_rounded.json');n=self.read('observations/rectangle_rounded_rhino_20261017.json')
        self.assertEqual(len(q['operations']),62);self.assertEqual(len(n['results']),62)
        exceptions={'rounded-arc-6','tiny-9'};matched=0
        for op,row in zip(q['operations'],n['results']):
            self.assertEqual(op['id'],row['id']);v=row['value'];self.assertIs(v['command_returned'],True)
            self.assertEqual([e['result']for e in v['events']if e['name']=='Rectangle'],['Success'])
            for token in ['_Cancel','Unknown command','Invalid']:self.assertNotIn(token,v['history'])
            self.assertEqual(v['after'],v['after_script'])
            self.assertEqual([e['objects']for e in v['events']if e['name']=='Rectangle'],[v['after_script']])
            for stage in ['before','after_script','undo','redo']:self.assertIs(v[stage]['guide_unchanged'],True)
            if op['id']in exceptions:
                self.assertEqual(v['after_script']['outputs'],[]);self.assertEqual(v['before'],v['after_script']);self.assertEqual(v['undo'],v['before']);self.assertEqual(v['redo'],v['before']);continue
            matched+=1
            self.assertTrue(v['undo_returned']);self.assertTrue(v['redo_returned']);self.assertEqual(v['before'],v['undo']);self.assertEqual(v['after_script'],v['redo'])
            self.assertEqual(len(v['after_script']['outputs']),1)
            witness=v['after_script']['outputs'][0];self.assertTrue(witness['closed']);self.assertTrue(math.isfinite(witness['length']))
        self.assertEqual(matched,60)
