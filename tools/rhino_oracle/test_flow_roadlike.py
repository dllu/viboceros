"""Roadlike explicit-axis qualification and retained unsupported native modes."""
import copy,json,math
from pathlib import Path
from unittest import TestCase
from .flow_roadlike_probe import validate
from .flow_line_comparison import comparison
BASE=Path(__file__).parent


class FlowRoadlikeTests(TestCase):
    def read(self,path):return json.loads((BASE/path).read_text())
    def request(self,name='flow_roadlike'):return self.read('fixtures/'+name+'.json')
    def observations(self,engine,name='flow_roadlike'):return self.read('observations/'+name+'_'+engine+'_20261016.json')

    def test_bounded_authored_axes_and_diagnostic_flags(self):
        for name,count in [('flow_roadlike',40),('flow_roadlike_diagnostics',12)]:
            q=self.request(name);self.assertEqual(len(q['operations']),count)
            for op in q['operations']:validate(op)
        op=self.request()['operations'][0]
        for name,value in [('base_up',[0,0,0]),('target_up',[0,False,1]),('cplane_up',[0,0,float('nan')]),('base_up',[0,1e7,0]),('base_up',[0,10**1000,0]),('target_up',[0,1]),('native_automatic',1),('unexpected',True)]:
            changed=copy.deepcopy(op);changed[name]=value
            with self.assertRaises(ValueError,msg=name):validate(changed)

    def test_complete_native_commands_use_picked_axes_and_replay_history(self):
        q=self.request();n=self.observations('rhino');self.assertEqual(len(n['results']),40)
        for op,row in zip(q['operations'],n['results']):
            self.assertEqual(op['id'],row['id']);v=row['value']
            self.assertTrue(v['command_returned']);self.assertTrue(v['undo_command_returned']);self.assertTrue(v['redo_command_returned'])
            self.assertIn('_Roadlike=_Yes',v['macro']);self.assertNotIn('_Automatic',v['macro']);self.assertNotIn('_Cancel',v['macro'])
            self.assertNotIn('Unknown command',v['history']);self.assertEqual(sum(e['name']=='Flow'for e in v['events']),1)
            for stage in ('before','after_script','undo','redo'):
                s=v[stage];self.assertTrue(s['guide_unchanged']);self.assertFalse(s['guide_selected'])
                refs=int(not op.get('base_line',True))+int(not op.get('target_line',False))
                self.assertEqual(s['object_count'],refs+len(s['outputs']))

    def test_full_owner_comparison_detects_axis_geometry_and_history_changes(self):
        q=self.request();a=self.observations('viboceros');n=self.observations('rhino')
        r=comparison(q,a,n);self.assertEqual(r,self.observations('comparison'));self.assertEqual(r['matched_cases'],40)
        for stage in ('after_script','undo','redo'):
            changed=copy.deepcopy(n);changed['results'][0]['value'][stage]['outputs'][0]['points'][0][2]+=.1
            self.assertEqual(comparison(q,a,changed)['matched_cases'],39)

    def test_unqualified_native_modes_remain_visible_and_rejected_atomically(self):
        name='flow_roadlike_diagnostics';q=self.request(name);a=self.observations('viboceros',name);n=self.observations('rhino',name)
        r=comparison(q,a,n);self.assertEqual(r,self.observations('comparison',name));self.assertEqual(r['matched_cases'],8)
        self.assertEqual(sum(bool(o.get('native_automatic'))for o in q['operations']),4)
        for op,x,y in zip(q['operations'],a['results'],n['results']):
            self.assertEqual(op['id'],x['id']);self.assertEqual(x['id'],y['id']);xx,yy=x['value'],y['value']
            self.assertTrue(yy['command_returned'])
            if not op.get('native_automatic'):
                self.assertTrue(xx['command_returned']);continue
            self.assertFalse(xx['command_returned']);self.assertIn('Roadlike',xx['error'])
            for stage in ('after','undo','redo'):self.assertEqual(xx['before'],xx[stage])
            self.assertIsNone(xx['undo_label']);self.assertIsNone(xx['redo_label'])

    def test_in_plane_and_nurbs_poles_match_complete_native_workflows(self):
        name='flow_roadlike_poles';q=self.request(name);a=self.observations('viboceros',name);n=self.observations('rhino',name)
        for o in q['operations']:validate(o)
        r=comparison(q,a,n);self.assertEqual(r,self.observations('comparison',name));self.assertEqual(r['cases'],40);self.assertEqual(r['matched_cases'],40)
        for op,x,y,row in zip(q['operations'],a['results'],n['results'],r['results']):
            self.assertEqual(op['id'],x['id']);self.assertEqual(x['id'],y['id']);self.assertTrue(y['value']['command_returned'])
            self.assertTrue(x['value']['command_returned'])
            self.assertTrue(row['within_1e_minus_6'],op['id']);self.assertNotIn('_Cancel',y['value']['macro'])
        self.assertEqual(sum(op['target']['type']=='nurbs'for op in q['operations']),4)
        # Axis-parallel maps are rank one; native Rigid retains its original pose.
        for key in ['rigid-pole-8','rigid-source-pole-16']:
            v=next(r['value']for r in n['results']if r['id']==key)
            self.assertEqual(v['before'],v['after_script'])

    def test_lookup_diagnostics_match_the_independent_inverse_and_preserve_automatic_rejections(self):
        from .flow_pole_report import report
        actual=report();self.assertEqual(actual,self.read('observations/flow_pole_report_20261016.json'))
        self.assertEqual(actual['native_commands'],52);self.assertEqual(actual['qualified_commands'],48)
        self.assertTrue(all(row['local_rejection_atomic']and row['native_command_succeeded']for group in actual['workflows']for row in group['rejected']))
        self.assertEqual(len(actual['lookup_diagnostics']),2)
        for row in actual['lookup_diagnostics']:
            self.assertTrue(row['within_document_tolerance'])
            self.assertLess(row['kernel_max_coordinate_error'],1e-6)
            self.assertLess(row['public_840_prediction_max_coordinate_error'],1e-12)
        self.assertFalse(actual['native_values_used_as_kernel_inputs'])
