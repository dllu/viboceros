import copy
from unittest import TestCase
from .curvature_sampling_probe import validate
from .curvature_sampling_comparison import BASE,CAPTURES,cases,comparison,local_request,read
GROUPS=CAPTURES+('curvature_sampling_angles','curvature_sampling_stress')

class CurvatureChordTests(TestCase):
    def test_combined_fixture_uses_only_original_recipe_geometry(self):
        self.assertEqual(local_request(GROUPS),read(BASE/'fixtures/curvature_chord_validation.json'))
        for capture in GROUPS[-2:]:
            for op in read(BASE/'fixtures'/(capture+'.json'))['operations']:validate(op)
        for local,(id,source,_)in zip(local_request(GROUPS)['operations'],cases(GROUPS)):
            self.assertEqual(local,dict(op='curvature_graph',id=id,source=source['source'],scale=source['scale'],density=source['density']))

    def test_native_outlines_close_original_gaps_and_hidden_wiggles(self):
        value=read(BASE/'observations/curvature_chord_comparison_20261016.json')
        for row in value['results']:
            if row['id'].startswith(('curvature_sampling_holdout/','curvature_sampling_angles/'))or'/hidden-'in row['id']:
                self.assertTrue(row['vertex_sequences_within_1e_minus_6'],row['id'])
        original=[r for r in value['results']if r['id'].split('/',1)[0]in CAPTURES]
        self.assertEqual(sum(r['vertex_sequences_within_1e_minus_6']for r in original),41)

    def test_retained_comparison_preserves_weighted_counterexamples(self):
        local=read(BASE/'observations/curvature_chord_viboceros_20261016.json')
        actual=comparison(local,GROUPS)
        self.assertEqual(actual,read(BASE/'observations/curvature_chord_comparison_20261016.json'))
        self.assertEqual(actual['cases'],72)
        self.assertEqual(actual['settings_applied_cases'],69)
        self.assertEqual(actual['matched_vertex_sequence_cases'],67)
        self.assertEqual(actual['unchanged_source_cases'],72)
        gaps={r['id']for r in actual['results']if r['requested_settings_applied']and not r['vertex_sequences_within_1e_minus_6']}
        self.assertEqual(gaps,{'curvature_sampling_stress/random-degree-3-2','curvature_sampling_stress/random-degree-5-2'})
        self.assertFalse(actual['complete_native_graph_parity_qualified'])
        altered=copy.deepcopy(local);altered['results'][0]['value']['outputs'][0]['points'][0][2]+=1.
        self.assertLess(comparison(altered,GROUPS)['matched_vertex_sequence_cases'],67)
