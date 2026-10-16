import copy
import json
from pathlib import Path
from unittest import TestCase
from .orient_points_probe import validate
from .orient_points_comparison import comparison
BASE=Path(__file__).parent


class PointOrientationTests(TestCase):
    def test_native_recipes_validate(self):
        for name in ('orient_points','orient_points_rotation','orient_points_memory','orient_points_discovery'):
            for op in json.loads((BASE/'fixtures'/(name+'.json')).read_text())['operations']:validate(op)

    def test_malformed_recipe_rejected(self):
        base=json.loads((BASE/'fixtures/orient_points.json').read_text())['operations'][0]
        for key,value in [('copy',1),('references',[]),('targets',[[0,0,0],[True,0,0]]),('scale','Yes'),('id','bad\nPICK'),('cancel_after',4),('batch_targets',[base['targets']]),('omit_scale',True)]:
            changed=copy.deepcopy(base);changed[key]=value
            with self.assertRaises(ValueError):validate(changed)

    def test_recorded_comparisons_detect_geometry_selection_groups_and_return_tampering(self):
        for name,count in [('orient_points',36),('orient_points_rotation',13)]:
            request=json.loads((BASE/'fixtures'/(name+'.json')).read_text())
            read=lambda suffix:json.loads((BASE/'observations'/(name+'_'+suffix+'_20261016.json')).read_text())
            actual,reference=read('viboceros'),read('rhino')
            result=comparison(request,actual,reference)
            self.assertEqual(result,read('comparison'));self.assertEqual((result['cases'],result['matched_cases']),(count,count))
            for mutation in ('point','selected','group','returned'):
                changed=copy.deepcopy(reference);v=changed['results'][0]['value']
                if mutation=='point':v['outputs'][0]['points'][2][1]+=.01
                elif mutation=='selected':v['outputs'][0]['selected']=not v['outputs'][0]['selected']
                elif mutation=='group':v['group_count']+=1
                else:v['command_returned']=False
                self.assertEqual(comparison(request,actual,changed)['matched_cases'],count-1)
            changed=copy.deepcopy(actual);changed['results'][0]['value']['outputs'][0]['points'][0][0]=float('nan')
            self.assertEqual(comparison(request,changed,reference)['matched_cases'],count-1)
            changed=copy.deepcopy(actual);changed['results'][0]['id']=changed['results'][1]['id']
            with self.assertRaises(ValueError):comparison(request,changed,reference)

    def test_cancelled_model_is_checked_independently_of_native_runscript_boolean(self):
        request=json.loads((BASE/'fixtures/orient_points.json').read_text())
        read=lambda suffix:json.loads((BASE/'observations'/('orient_points_'+suffix+'_20261016.json')).read_text())
        actual,reference=read('viboceros'),read('rhino')
        index=next(i for i,op in enumerate(request['operations'])if op['id']=='Orient3Pt-cancel-target')
        self.assertTrue(reference['results'][index]['value']['command_returned'])
        reference['results'][index]['value']['command_returned']=False
        self.assertEqual(comparison(request,actual,reference)['matched_cases'],36)
