"""Temporary portions are command inputs; whole-source SDK calls are separate."""
import copy,json
from pathlib import Path
from unittest import TestCase
from .curve_deviation_probe import validate
from .curve_deviation_comparison import comparison

BASE=Path(__file__).parent
def read(path):return json.loads((BASE/path).read_text())

class CurveDeviationSubcurveTests(TestCase):
    def request(self):return read('fixtures/curve_deviation_subcurves.json')
    def test_subcurve_recipes_reject_ambiguous_types_and_out_of_domain_inputs(self):
        q=self.request();self.assertEqual(len(q['operations']),9)
        for op in q['operations']:validate(op)
        op=q['operations'][0]
        for ranges in [None,[],[None],[[0,0],None],[[True,1],None],[[-.1,.5],None],[[0,float('nan')],None],[[0,1.1],None]]:
            with self.subTest(ranges=ranges),self.assertRaises(ValueError):validate(dict(op,subcurves=ranges))
        with self.assertRaises(ValueError):validate(dict(op,postselect=False))
        missing=dict(op);missing.pop('keep_marks')
        with self.assertRaises(ValueError):validate(missing)
    def test_native_getter_preserves_source_definitions_and_reverse_parameter_domains(self):
        native=read('observations/curve_deviation_subcurves_rhino_20261016.json')
        for op,row in zip(self.request()['operations'],native['results']):
            self.assertEqual(op['id'],row['id']);v=row['value']
            self.assertTrue(v['inputs_retained']);self.assertTrue(v['inputs_unchanged'])
            self.assertEqual(v['sources_before'],v['sources_after']);self.assertEqual(v['sdk_scope'],'whole_sources')
            self.assertIn('_SubCrv',v['command_history']);self.assertIn('Overlap interval',v['command_history'])
            self.assertEqual(v['created_object_count'],len(v['outputs']))
        reverse=next(r['value']for r in native['results']if r['id']=='reverse-open')
        self.assertIn('crvA(        -7)',reverse['command_history']);self.assertIn('crvA(        -3)',reverse['command_history'])
    def test_subcurve_comparison_retains_command_and_marker_differences(self):
        q=self.request();local=read('observations/curve_deviation_subcurves_viboceros_20261016.json');native=read('observations/curve_deviation_subcurves_rhino_20261016.json')
        expected=read('observations/curve_deviation_subcurves_comparison_20261016.json')
        result=comparison(q,local,native,interval_ends=True);self.assertEqual(result,expected)
        self.assertEqual(result['counts']['status_matches'],9)
        self.assertEqual(result['counts']['sdk_maximum_scalar_matches'],0)
        self.assertEqual(result['counts']['sdk_minimum_scalar_matches'],0)
        self.assertFalse(result['complete_native_command_parity_qualified'])
        self.assertTrue(all(not r['sdk_scalars_compared']for r in result['results']))
        self.assertTrue(all(r['value']['inputs_unchanged']for r in local['results']))
    def test_whole_source_sdk_values_cannot_qualify_subcurve_commands(self):
        q=self.request();local=read('observations/curve_deviation_subcurves_viboceros_20261016.json');native=read('observations/curve_deviation_subcurves_rhino_20261016.json')
        expected=comparison(q,local,native,interval_ends=True)
        changed=copy.deepcopy(native)
        for row in changed['results']:
            for key in ('minimum','maximum'):
                if row['value']['sdk'][key]is not None:row['value']['sdk'][key]['distance']=1e99
        self.assertEqual(comparison(q,local,changed,interval_ends=True),expected)
        changed['results'][0]['value']['sdk_scope']='measured_inputs'
        with self.assertRaises(ValueError):comparison(q,local,changed,interval_ends=True)
