import copy,json
from pathlib import Path
from unittest import TestCase
from .curvature_graph_probe import validate
from .curvature_graph_comparison import comparison
BASE=Path(__file__).parent
def read(p):return json.loads((BASE/p).read_text())
class CurvatureGraphTests(TestCase):
    def test_recipe_validation_is_strict(self):
        q=read('fixtures/curvature_graph.json')
        for op in q['operations']:validate(op)
        op=q['operations'][0]
        for key,value in [('scale',True),('scale',301),('scale',float('nan')),('density',-1),('density',11),('density','2'),('unexpected',0)]:
            with self.subTest(key=key,value=value),self.assertRaises(ValueError):validate(dict(op,**{key:value}))
    def test_public_extracted_circle_graphs_confirm_exponential_scale(self):
        q=read('fixtures/curvature_graph_discovery.json');r=read('observations/curvature_graph_discovery_rhino_20261016.json')
        import math
        for op,row in zip(q['operations'],r['results']):
            v=row['value'];self.assertTrue(v['command_returned']);self.assertTrue(v['extraction_returned']);self.assertTrue(v['inputs_unchanged'])
            self.assertEqual(len(v['outputs']),1)
            expected=op['source']['radius']+2.**((op['scale']-100.)/2.)/op['source']['radius']
            for p in v['outputs'][0]['points']:self.assertLess(abs(math.dist(p,op['source']['center'])-expected)/expected,1e-12)
    def test_comparison_preserves_independent_sampling_and_detects_changed_vectors(self):
        q=read('fixtures/curvature_graph.json');local=read('observations/curvature_graph_viboceros_20261016.json');native=read('observations/curvature_graph_rhino_20261016.json')
        result=comparison(q,local,native);self.assertEqual(result,read('observations/curvature_graph_comparison_20261016.json'))
        self.assertEqual(result['matched_sample_cases'],9);self.assertEqual(result['unchanged_source_cases'],9)
        self.assertFalse(result['complete_native_graph_parity_qualified'])
        altered=copy.deepcopy(local);altered['results'][0]['value']['samples'][0]['curvature'][0]+=1.
        self.assertEqual(comparison(q,altered,native)['matched_sample_cases'],8)
