import copy
import json
from pathlib import Path
from unittest import TestCase
from .orient_on_curve_probe import validate
from .orient_on_curve_comparison import comparison

BASE = Path(__file__).parent


class OrientOnCurveTests(TestCase):
    def test_partial_guide_and_relative_input_validation(self):
        recipe = json.loads((BASE/'fixtures/orient_on_curve_subcurve.json').read_text())['operations'][0]
        for sub in (None, {}, {'start':[0,1,0],'end':[0,2]}, {'start':[0,1,0],'end':[0,2,0],'length':True}, {'start':[0,1,0],'end':[0,2,0],'length':float('nan')}):
            changed=copy.deepcopy(recipe);changed['subcurve']=sub
            with self.assertRaises(ValueError):validate(changed)
        changed=copy.deepcopy(recipe);changed['relative_target']=[0,-1,0]
        with self.assertRaises(ValueError):validate(changed)
        changed['options']['copy']=True;validate(changed)
    def test_native_recipes_validate(self):
        for name in ('orient_on_curve', 'orient_on_curve_spatial', 'orient_on_curve_subcurve', 'orient_on_curve_projected_input', 'orient_on_curve_relative_input'):
            for op in json.loads((BASE/'fixtures'/(name+'.json')).read_text())['operations']:
                validate(op)

    def test_invalid_recipes_are_rejected(self):
        recipe = json.loads((BASE/'fixtures/orient_on_curve.json').read_text())['operations'][0]
        for field, value in [('base', [True,0,0]), ('target', [float('inf'),0,0]),
                             ('options', {'copy': 1}), ('options', {'rotate': float('nan')}),
                             ('options', {'unknown': True}), ('plane', 'unknown')]:
            changed = copy.deepcopy(recipe)
            changed[field] = value
            with self.assertRaises(ValueError):
                validate(changed)

    def test_retained_comparisons_cover_all_witnesses_and_detect_tampering(self):
        for name, count in [('orient_on_curve',35), ('orient_on_curve_spatial',14), ('orient_on_curve_subcurve',26), ('orient_on_curve_projected_input',4), ('orient_on_curve_relative_input',2)]:
            read = lambda stem: json.loads((BASE/'observations'/(stem+'_20261016.json')).read_text())
            local, native = read(name+'_viboceros'), read(name+'_rhino')
            result = comparison(local, native)
            self.assertEqual(result, read(name+'_comparison'))
            self.assertEqual((result['cases'], result['matched_cases']), (count,count))
            changed = copy.deepcopy(local)
            changed['results'][0]['value']['outputs'][0]['points'][1][2] += .01
            self.assertEqual(comparison(changed,native)['matched_cases'], count-1)
            changed = copy.deepcopy(local)
            changed['results'][0]['value']['curve_unchanged'] = False
            self.assertEqual(comparison(changed,native)['matched_cases'], count-1)
            changed = copy.deepcopy(local)
            changed['results'][0]['value']['outputs'][0]['points'][1][2] = float('nan')
            self.assertEqual(comparison(changed,native)['matched_cases'], count-1)
            changed = copy.deepcopy(local)
            changed['results'][0]['id'] = changed['results'][1]['id']
            with self.assertRaises(ValueError): comparison(changed,native)
