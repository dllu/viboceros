import copy
import json
from pathlib import Path
from unittest import TestCase
from .flow_grips_probe import validate
from .flow_grips_comparison import comparison
BASE=Path(__file__).parent


class FlowGripTests(TestCase):
    def request(self):return json.loads((BASE/'fixtures/flow_grips.json').read_text())
    def read(self,name):return json.loads((BASE/'observations'/('flow_grips_'+name+'_20261016.json')).read_text())
    def test_bounded_authored_flow_grip_inputs(self):
        request=self.request();self.assertEqual(len(request['operations']),72)
        for op in request['operations']:validate(op)
        original=request['operations'][0]
        for key,value in [('selected',[]),('selected',[True]),('selected',[1,1]),('selected',[-1]),('selected',[64]),('parent',1),('peer',1),('edit',1),('postselected',1),('id','bad\nPICK'),('source',dict(type='point',point=[0,0,0])),('options',dict(roadlike=True))]:
            op=copy.deepcopy(original);op[key]=value
            with self.assertRaises(ValueError,msg=key):validate(op)
    def test_capture_completes_each_command_and_replays_real_history(self):
        native=self.read('rhino');self.assertEqual(len(native['results']),72)
        for row in native['results']:
            value=row['value'];self.assertTrue(value['command_returned']);self.assertTrue(value['undo_command_returned']);self.assertTrue(value['redo_command_returned'])
            self.assertNotIn('Unknown command',value['history']);self.assertNotIn('_Cancel',value['macro'])
            self.assertEqual(sum(e['name']=='Flow'for e in value['events']),1)
            for key in ('after_script','undo','redo'):
                self.assertTrue(value[key]['guides_unchanged']);self.assertEqual(value[key]['guides_selected'],[False,False])
            # Undo must actually remove copies or restore source coordinates.
            self.assertNotEqual(value['after_script']['outputs'],value['undo']['outputs'],row['id'])
    def test_retained_comparison_checks_owner_grips_selection_groups_and_history(self):
        local,native=self.read('viboceros'),self.read('rhino');report=comparison(self.request(),local,native)
        self.assertEqual(report,self.read('comparison'));self.assertEqual(report['matched_cases'],72)
        for field in ('control','weight','knot','selection','grip','group','guide','topology','nonfinite','undo'):
            changed=copy.deepcopy(native);value=changed['results'][0]['value'];stage=value['undo']if field=='undo'else value['after_script'];owner=next(o for o in stage['outputs']if 'curve'in o)
            if field in('control','undo'):owner['curve']['control_points'][1]['point'][1]+=.1
            elif field=='weight':owner['curve']['control_points'][1]['weight']+=.1
            elif field=='knot':owner['curve']['knots'][3]+=.01
            elif field=='selection':owner['grips'][0]['selected']=not owner['grips'][0]['selected']
            elif field=='grip':owner['grips'][1]['point'][2]+=.1
            elif field=='group':owner['groups']=[9]
            elif field=='guide':stage['guides_unchanged']=False
            elif field=='topology':owner['curve']['degree']+=1
            else:owner['curve']['control_points'][0]['point'][0]=float('nan')
            self.assertEqual(comparison(self.request(),local,changed)['matched_cases'],71,field)
        changed=copy.deepcopy(local);changed['results'][0]['id']=changed['results'][1]['id']
        with self.assertRaises(ValueError):comparison(self.request(),changed,native)
