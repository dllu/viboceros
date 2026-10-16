import copy
import json
from pathlib import Path
from unittest import TestCase
from .orient_surface_grips_probe import validate
from .orient_surface_grips_comparison import comparison
BASE=Path(__file__).parent
class SurfaceGripTests(TestCase):
    def test_authored_grip_recipes_validate(self):
        rows=json.loads((BASE/'fixtures/orient_surface_grips.json').read_text())['operations']
        self.assertEqual(len(rows),30)
        for row in rows:validate(row)
    def test_malformed_bounded_grip_input_is_rejected(self):
        r=json.loads((BASE/'fixtures/orient_surface_grips.json').read_text())['operations'][0]
        for key,value in [('selected',[]),('selected',[True]),('selected',[1,1]),('selected',[-1]),('selected',[64]),('source',dict(type='point',point=[0,0,0])),('parent',1),('edit',1),('postselected',1),('id','bad\nPICK')]:
            row=copy.deepcopy(r);row[key]=value
            with self.assertRaises(ValueError):validate(row)
    def test_retained_comparison_detects_owner_grip_and_group_corruption(self):
        request=json.loads((BASE/'fixtures/orient_surface_grips.json').read_text());read=lambda name:json.loads((BASE/'observations'/('orient_surface_grips_'+name+'_20261016.json')).read_text());local,native=read('viboceros'),read('rhino');report=comparison(request,local,native)
        self.assertEqual(report,read('comparison'));self.assertEqual(report['matched_cases'],30)
        for field in ('control','weight','knots','selection','grip','group','guide','topology','nonfinite'):
            changed=copy.deepcopy(native);v=changed['results'][0]['value']['after'];o=v['outputs'][0]
            if field=='control':o['curve']['control_points'][1]['point'][1]+=.1
            elif field=='weight':o['curve']['control_points'][1]['weight']+=.1
            elif field=='knots':o['curve']['knots'][3]+=.01
            elif field=='selection':o['grips'][0]['selected']=not o['grips'][0]['selected']
            elif field=='grip':o['grips'][1]['point'][2]+=.1
            elif field=='group':o['groups']=[9]
            elif field=='guide':v['guide_unchanged']=False
            elif field=='topology':o['curve']['degree']+=1
            else:o['curve']['control_points'][0]['point'][0]=float('nan')
            self.assertEqual(comparison(request,local,changed)['matched_cases'],29,field)
        changed=copy.deepcopy(local);changed['results'][0]['id']=changed['results'][1]['id']
        with self.assertRaises(ValueError):comparison(request,changed,native)
