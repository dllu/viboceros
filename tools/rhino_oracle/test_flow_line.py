import copy
import json
from pathlib import Path
from unittest import TestCase
from .flow_line_probe import validate
from .flow_line_comparison import comparison
BASE=Path(__file__).parent


class FlowLineTests(TestCase):
    def request(self):return json.loads((BASE/'fixtures/flow_line.json').read_text())
    def read(self,name):return json.loads((BASE/'observations'/('flow_line_'+name+'_20261016.json')).read_text())
    def test_authored_line_inputs_and_source_families(self):
        request=self.request();self.assertEqual(len(request['operations']),128)
        for op in request['operations']:validate(op)
        op=request['operations'][0]
        for key,value in [('id','bad\nPICK'),('base_line',1),('target_line',1),('edit',1),('selected',[True]),('selected',[1,1]),('options',dict(roadlike=True))]:
            changed=copy.deepcopy(op);changed[key]=value
            with self.assertRaises(ValueError,msg=key):validate(changed)
        for start,end in [([0,0,0],[0,0,0]),([0,0],[1,2,3]),([False,0,0],[1,0,0]),([0,0,0],[float('nan'),0,0]),([-1e308,0,0],[1e308,0,0])]:
            changed=copy.deepcopy(op);changed['base']=dict(type='line',start=start,end=end)
            with self.assertRaises(ValueError):validate(changed)
    def test_real_commands_keep_drawn_guides_out_of_document_and_replay_history(self):
        request=self.request();native=self.read('rhino');self.assertEqual(len(native['results']),128)
        for op,row in zip(request['operations'],native['results']):
            self.assertEqual(op['id'],row['id']);v=row['value']
            self.assertTrue(v['command_returned']);self.assertTrue(v['undo_command_returned']);self.assertTrue(v['redo_command_returned'])
            self.assertNotIn('_Cancel',v['macro']);self.assertNotIn('Unknown command',v['history'])
            self.assertEqual(sum(e['name']=='Flow'for e in v['events']),1)
            refs=int(not op.get('base_line',True))+int(not op.get('target_line',False))
            for stage in ('before','after_script','undo','redo'):
                state=v[stage];self.assertTrue(state['guide_unchanged']);self.assertFalse(state['guide_selected'])
                self.assertEqual(state['object_count'],refs+len(state['outputs']),row['id'])
            self.assertNotEqual(v['after_script']['outputs'],v['undo']['outputs'],row['id'])
    def test_comparison_detects_lifetime_geometry_selection_group_and_history_changes(self):
        request=self.request();local,native=self.read('viboceros'),self.read('rhino');report=comparison(request,local,native)
        self.assertEqual(report,self.read('comparison'));self.assertEqual(report['matched_cases'],128)
        for field in ('point','selection','group','guide','object_count','undo','nonfinite'):
            changed=copy.deepcopy(native);v=changed['results'][0]['value'];s=v['undo']if field=='undo'else v['after_script'];o=s['outputs'][0]
            if field in ('point','undo'):o['points'][0][0]+=.1
            elif field=='selection':o['selected']=not o['selected']
            elif field=='group':o['groups']=[99]
            elif field=='guide':s['guide_unchanged']=False
            elif field=='object_count':s['object_count']+=1
            else:o['points'][0][0]=float('nan')
            self.assertEqual(comparison(request,local,changed)['matched_cases'],127,field)
    def test_parallel_collinear_and_nearly_opposite_orientation_diagnostics(self):
        read=lambda p:json.loads((BASE/p).read_text())
        request=read('fixtures/flow_line_orientation.json')
        local=read('observations/flow_line_orientation_viboceros_20261016.json')
        native=read('observations/flow_line_orientation_rhino_20261016.json')
        report=comparison(request,local,native)
        self.assertEqual(report,read('observations/flow_line_orientation_comparison_20261016.json'))
        self.assertEqual(report['matched_cases'],24)
        self.assertEqual(sum(o['op']=='flow_line'for o in request['operations']),20)
        for op in request['operations']:
            if op['op']=='flow_line':validate(op)
        sdk=next(v for v in native['results']if 'sdk_points'in v['value'])
        sdk['value']['sdk_points'][0][0]+=.1
        self.assertEqual(comparison(request,local,native)['matched_cases'],23)
