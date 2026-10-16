import copy
import json
from pathlib import Path
from unittest import TestCase
from .orient_surface_probe import validate
from .orient_surface_comparison import comparison
BASE=Path(__file__).parent

class SurfaceOrientationTests(TestCase):
    def test_authored_surface_recipes_validate(self):
        for name in ('orient_surface','orient_surface_policy'):
            for op in json.loads((BASE/'fixtures'/(name+'.json')).read_text())['operations']:validate(op)

    def test_malformed_and_unbounded_native_driver_inputs_are_rejected(self):
        original=json.loads((BASE/'fixtures/orient_surface.json').read_text())['operations'][0]
        for key,value in [('id','bad\nPICK'),('uv',[True,0]),('options',{'scale':0}),('options',{'scale':float('inf')}),('options',{'rigid':True,'constrain_normal':True}),('options',{'copy':1}),('options',{'unknown':True}),('offsets',[[0,0,0]]),('offsets',[[0,0,0]]*33),('offsets',[[0,0,0]]*3+[[0,True,0]]),('batch',[dict(uv=[0,1],scale=-1)]),('batch',[dict(uv=[0,1])]*4),('batch',[dict(uv=[0,1],rotation=float('nan'))]),('batch',[dict(uv=[True,1])])]:
            op=copy.deepcopy(original);op[key]=value
            with self.assertRaises(ValueError,msg=key):validate(op)

    def test_retained_model_comparisons_detect_tampered_geometry_and_metadata(self):
        for name,count in [('orient_surface',22),('orient_surface_policy',10)]:
            request=json.loads((BASE/'fixtures'/(name+'.json')).read_text())
            read=lambda suffix:json.loads((BASE/'observations'/(name+'_'+suffix+'_20261016.json')).read_text())
            local,native=read('viboceros'),read('rhino');result=comparison(request,local,native)
            self.assertEqual((result['cases'],result['matched_cases']),(count,count));self.assertEqual(result,read('comparison'))
            for field in ('point','selected','groups','guide','returned','nonfinite','point_count'):
                changed=copy.deepcopy(native);v=changed['results'][0]['value']
                if field=='point':v['outputs'][0]['points'][2][0]+=.1
                elif field=='selected':v['outputs'][0]['selected']=not v['outputs'][0]['selected']
                elif field=='groups':v['group_count']+=1
                elif field=='guide':v['guide_unchanged']=False
                elif field=='returned':v['command_returned']=False
                elif field=='nonfinite':v['outputs'][0]['points'][0][0]=float('nan')
                else:v['outputs'][0]['points'].pop()
                self.assertEqual(comparison(request,local,changed)['matched_cases'],count-1,field)
            changed=copy.deepcopy(local);changed['results'][0]['id']=changed['results'][1]['id']
            with self.assertRaises(ValueError):comparison(request,changed,native)
