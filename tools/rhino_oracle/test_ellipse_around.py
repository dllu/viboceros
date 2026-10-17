"""Owned around-curve inputs and native geometry/history receipts."""
import json
from pathlib import Path
from unittest import TestCase
from .ellipse_probe import validate
BASE=Path(__file__).parent


class EllipseAroundTests(TestCase):
    def test_native_normal_planes_sources_component_clicks_and_selection_replay(self):
        total=edges=0
        for profile,count in [('command',8),('extended',19)]:
            q=json.loads((BASE/('fixtures/ellipse_around_'+profile+'.json')).read_text())
            n=json.loads((BASE/('observations/ellipse_around_'+profile+'_rhino_20261017.json')).read_text())
            self.assertEqual(len(q['operations']),count);self.assertEqual(len(n['results']),count)
            for op,row in zip(q['operations'],n['results']):
                validate(op);self.assertEqual(op['id'],row['id']);v=row['value'];trace=v['input_trace']
                self.assertEqual([e['result']for e in v['events']if e['name']=='Ellipse'],['Success'])
                self.assertEqual(trace['errors'],[]);self.assertEqual(trace['completed'],len(op['actions']));self.assertEqual(trace['expected'],len(op['actions']));self.assertTrue(all(trace['prompts']))
                self.assertEqual(v['after']['outputs'],v['after_script']['outputs']);self.assertEqual(v['after']['outputs'],v['redo']['outputs']);self.assertEqual(v['before']['outputs'],v['undo']['outputs'])
                self.assertTrue(v['undo_returned']);self.assertTrue(v['redo_returned'])
                picks=[a for a in op['actions']if isinstance(a,dict)and'edge'in a]
                self.assertEqual(len(trace['edge_picks']),len(picks))
                for receipt,pick in zip(trace['edge_picks'],picks):
                    self.assertTrue(receipt['acknowledged']);self.assertEqual(receipt['authored_edge'],pick['edge']);self.assertEqual(receipt['pick'],pick['pick']);edges+=1
                self.assertEqual(len(v['after']['outputs']),1);self.assertFalse(v['after']['outputs'][0]['selected'])
                before=op.get('preselect',False)
                for stage in ['before','after','after_script','undo','redo']:
                    self.assertTrue(v[stage]['guide_unchanged'])
                    self.assertEqual(v[stage]['guide_selected'],[False if picks else True if stage=='after' else before])
                total+=1
        self.assertEqual((total,edges),(27,4))

    def test_bound_source_schema_and_component_actions_are_checked_before_launch(self):
        op=json.loads((BASE/'fixtures/ellipse_around_command.json').read_text())['operations'][0]
        for field,value in [
            ('curves',[dict(type='circle',center=[0,0,0],radius=1,normal=[0,0,1])]),
            ('curves',[dict(type='circle',center=[0,0,0],radius=1,x_axis=[1,0,0])]),
            ('curves',[dict(type='line',start=[0,0,0],end=[10**500,0,0])]),
            ('curves',op['curves']*5),('curves',[dict(type='mesh')]),
            ('actions',['AroundCurve',dict(curve=3)]),('actions',['AroundCurve',dict(curve=True)]),
            ('actions',['AroundCurve',dict(curve=0,edge=0,pick=[0,0,0])]),
            ('actions',['AroundCurve',dict(curve=0,unexpected=True)]),
            ('preselect',1),('guide_selection',1),
            ('setup',['AroundCurve',dict(curve=0)]),
        ]:
            with self.assertRaises(ValueError,msg=(field,value)):validate(dict(op,**{field:value}))
        surface=json.loads((BASE/'fixtures/ellipse_around_extended.json').read_text())['operations'][-1]
        for pick in [dict(curve=0),dict(curve=0,edge=64,pick=[2,.96,.64]),dict(curve=0,edge=0,pick=[0,0,True])]:
            with self.assertRaises(ValueError):validate(dict(surface,actions=['AroundCurve',pick]))
        with self.assertRaises(ValueError):validate(dict(surface,live_getters=False))

    def test_native_cancellation_cleanup_and_incomplete_retry_are_separate(self):
        q=json.loads((BASE/'fixtures/ellipse_around_controls.json').read_text());n=json.loads((BASE/'observations/ellipse_around_controls_rhino_20261017.json').read_text())
        self.assertEqual(len(q['operations']),6);outcomes=[]
        for op,row in zip(q['operations'],n['results']):
            validate(op);self.assertEqual(op['id'],row['id']);v=row['value'];t=v['input_trace'];outcomes.extend(e['result']for e in v['events']if e['name']=='Ellipse')
            self.assertEqual(t['errors'],[]);self.assertEqual(t['completed'],len(op['actions']))
            for stage in ['before','after','after_script','undo','redo']:
                self.assertEqual(v[stage]['outputs'],[]);self.assertTrue(v[stage]['guide_unchanged'])
            for stage in ['after_script','undo','redo']:self.assertEqual(v[stage]['guide_selected'],[op.get('preselect',False)])
        self.assertEqual(outcomes.count('Cancel'),5);self.assertEqual(outcomes.count('Failure'),1)
        diag=json.loads((BASE/'observations/ellipse_around_diagnostics_rhino_20261017.json').read_text())['results']
        self.assertEqual(len(diag),1);self.assertIn('owned Ellipse input timeout',diag[0]['value']['input_trace']['errors'])
