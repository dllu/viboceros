import copy
from unittest import TestCase
from .curvature_sampling_probe import validate
from .curvature_sampling_comparison import BASE,CAPTURES,cases,comparison,local_request,read,sequence_error

class CurvatureSamplingTests(TestCase):
    def test_native_recipes_and_sampling_limits_are_validated(self):
        for capture in CAPTURES+('curvature_sampling_quality',):
            for op in read(BASE/'fixtures'/(capture+'.json'))['operations']:validate(op)
        op=copy.deepcopy(cases()[0][1])
        for value in (True,0,11,float('nan'),'2'):
            with self.subTest(value=value),self.assertRaises(ValueError):validate(dict(op,sample_density=value))
        with self.assertRaises(ValueError):validate(dict(op,unexpected=0))

    def test_local_recipes_contain_only_original_source_inputs(self):
        expected=local_request()
        self.assertEqual(expected,read(BASE/'fixtures/curvature_sampling_validation.json'))
        for local,(id,source,_)in zip(expected['operations'],cases()):
            self.assertEqual(local,dict(op='curvature_graph',id=id,source=source['source'],scale=source['scale'],density=source['density']))

    def test_vertex_matching_allows_reversal_and_object_order(self):
        a=[dict(points=[[0,0,0],[1,0,0]]),dict(points=[[0,2,0],[1,2,0]])]
        b=copy.deepcopy([dict(points=list(reversed(p['points'])))for p in reversed(a)])
        self.assertEqual(sequence_error(a,b),0.)
        b[0]['points'][0][2]=.25
        self.assertEqual(sequence_error(a,b),.25)
        self.assertIsNone(sequence_error(a,b[:1]))
        self.assertEqual(sequence_error(a,[a[0],a[0]]),2.)
        self.assertIsNone(sequence_error(a,[a[0],dict(points=[[0,0,0],[1,0,0],[2,0,0]])]))

    def test_retained_comparison_reproduces_matches_and_remaining_gaps(self):
        local=read(BASE/'observations/curvature_sampling_viboceros_20261016.json')
        actual=comparison(local)
        self.assertEqual(actual,read(BASE/'observations/curvature_sampling_comparison_20261016.json'))
        self.assertEqual(actual['cases'],44)
        self.assertEqual(actual['settings_applied_cases'],41)
        self.assertEqual(actual['unchanged_source_cases'],44)
        self.assertFalse(actual['complete_native_graph_parity_qualified'])
        self.assertTrue(any(r['requested_settings_applied']and not r['vertex_sequences_within_1e_minus_6']for r in actual['results']))
        altered=copy.deepcopy(local)
        altered['results'][0]['value']['outputs'][0]['points'][0][2]+=1.
        self.assertLess(comparison(altered)['matched_vertex_sequence_cases'],actual['matched_vertex_sequence_cases'])
        altered=copy.deepcopy(local);altered['results'][0]['id']='wrong-case'
        with self.assertRaises(ValueError):comparison(altered)
        altered=copy.deepcopy(local);altered['results'][0]['value']['outputs'][0]['points'][0][0]=float('nan')
        with self.assertRaises(ValueError):comparison(altered)

    def test_ignored_outline_setting_and_wrong_branch_recovery_remain_visible(self):
        failed=read(BASE/'observations/curvature_sampling_quality_rhino_20261016.json')
        self.assertTrue(all(r['value']['sampling_after_setup']==2 for r in failed['results']))
        holdout=read(BASE/'observations/curvature_sampling_holdout_rhino_20261016.json')
        loop=next(r['value']for r in holdout['results']if r['id']=='cubic-loop')
        self.assertGreater(max(s['residual']for g in loop['graphs']for s in g['stations']),.18)
        self.assertTrue(loop['inputs_unchanged'])
