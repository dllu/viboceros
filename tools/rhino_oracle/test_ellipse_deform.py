"""Native periodic cubic Ellipse output, getter receipts and separate diagnostics."""
import json
from pathlib import Path
from unittest import TestCase
from .ellipse_probe import validate
BASE=Path(__file__).parent

class EllipseDeformTests(TestCase):
    def test_native_shapes_modes_counts_preferences_and_edge_receipts(self):
        total=cubics=edges=0
        for profile,count in [('shapes',9),('modes',20),('counts',4)]:
            q=json.loads((BASE/('fixtures/ellipse_deform_'+profile+'.json')).read_text())
            n=json.loads((BASE/('observations/ellipse_deform_'+profile+'_rhino_20261017.json')).read_text())
            self.assertEqual(len(q['operations']),count);self.assertEqual(len(n['results']),count)
            for op,row in zip(q['operations'],n['results']):
                validate(op);self.assertEqual(op['id'],row['id']);v=row['value'];t=v['input_trace']
                self.assertEqual([e['result']for e in v['events']if e['name']=='Ellipse'],['Success'])
                self.assertEqual(t['errors'],[]);self.assertEqual(t['completed'],t['expected']);self.assertEqual(t['completed'],len(op['actions']))
                self.assertNotIn('Unknown command',v['history'])
                self.assertEqual(v['before']['outputs'],v['undo']['outputs']);self.assertEqual(v['after']['outputs'],v['after_script']['outputs']);self.assertEqual(v['after']['outputs'],v['redo']['outputs']);self.assertTrue(v['undo_returned']);self.assertTrue(v['redo_returned'])
                for stage in ['before','after','after_script','undo','redo']:self.assertTrue(v[stage]['guide_unchanged'])
                if 'setup'in op:
                    self.assertEqual(v['setup_input_trace']['errors'],[]);self.assertEqual(v['setup_input_trace']['completed'],len(op['setup']))
                for obj in v['after']['outputs']:
                    self.assertFalse(obj['selected'])
                    if 'curve'not in obj:continue
                    c=obj['curve']
                    if op['id']=='ellipse-deform-reset-output':self.assertEqual(c['degree'],2);continue
                    cubics+=1;self.assertEqual(c['degree'],3);self.assertTrue(obj['closed']);self.assertTrue(all(p['weight']==1.0 for p in c['control_points']));self.assertEqual(c['control_points'][:3],c['control_points'][-3:])
                for receipt in t['edge_picks']:self.assertTrue(receipt['acknowledged']);edges+=1
                total+=1
        self.assertEqual((total,cubics,edges),(33,32,1))

    def test_clean_native_cancellations_and_unqualified_attempts_remain_separate(self):
        q=json.loads((BASE/'fixtures/ellipse_deform_controls.json').read_text());n=json.loads((BASE/'observations/ellipse_deform_controls_rhino_20261017.json').read_text())
        self.assertEqual(len(q['operations']),2)
        for op,row in zip(q['operations'],n['results']):
            validate(op);self.assertEqual(op['id'],row['id']);v=row['value'];self.assertEqual(v['input_trace']['errors'],[]);self.assertEqual(v['input_trace']['completed'],len(op['actions']));self.assertEqual([e['result']for e in v['events']if e['name']=='Ellipse'],['Cancel'])
            for stage in ['before','after','after_script','undo','redo']:self.assertEqual(v[stage]['outputs'],[])
        q=json.loads((BASE/'fixtures/ellipse_deform_diagnostics.json').read_text());n=json.loads((BASE/'observations/ellipse_deform_diagnostics_rhino_20261017.json').read_text())
        self.assertEqual(len(q['operations']),9);self.assertEqual(len(n['results']),9)
        unknown=timeouts=0
        for op,row in zip(q['operations'],n['results']):
            validate(op);self.assertEqual(op['id'],row['id']);v=row['value'];unknown+='Unknown command'in v['history'];timeouts+='owned Ellipse input timeout'in v['input_trace']['errors']
        self.assertEqual((unknown,timeouts),(6,2))

    def test_point_count_recipes_remain_bounded(self):
        op=dict(op='ellipse_command',id='bound-count',actions=['Deformable','PointCount=1000',[0,0,0],[6,0,0],[0,3,0]])
        validate(op)
        for option in ['PointCount=1001','PointCount=1000000','PointCount=3.5','PointCount=NaN','Degree=100']:
            with self.assertRaises(ValueError):validate(dict(op,actions=[option]))
