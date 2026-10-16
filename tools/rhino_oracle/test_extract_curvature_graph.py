import copy,json
from pathlib import Path
from unittest import TestCase
from .extract_curvature_graph_probe import validate
from .extract_curvature_graph_comparison import comparison
BASE=Path(__file__).parent
def read(p):return json.loads((BASE/p).read_text())
class ExtractCurvatureGraphTests(TestCase):
    def test_request_rejects_ambiguous_types_indices_and_flags(self):
        q=read('fixtures/extract_curvature_graph.json')
        for op in q['operations']:validate(op)
        op=q['operations'][0]
        for key,value in [('selected',[True]),('graphs_on',[[]]),('graphs_on',[0,0]),('selected',[2]),('scale',301),('density',-1),('postselect',1),('undo_redo',None)]:
            with self.subTest(key=key,value=value),self.assertRaises(ValueError):validate(dict(op,**{key:value}))
    def test_comparison_retains_policy_matches_and_geometry_and_undo_gaps(self):
        q=read('fixtures/extract_curvature_graph.json');local=read('observations/extract_curvature_graph_viboceros_20261016.json');native=read('observations/extract_curvature_graph_rhino_20261016.json')
        result=comparison(q,local,native);self.assertEqual(result,read('observations/extract_curvature_graph_comparison_20261016.json'))
        for key in ('command_status_matches','piece_count_matches','attributes_match','source_selection_matches','sources_unchanged'):self.assertEqual(result['counts'][key],10)
        self.assertFalse(result['native_polyline_geometry_qualified']);self.assertFalse(result['native_undo_redo_qualified']);self.assertFalse(result['continuous_geometry_distance_qualified'])
        self.assertLess(result['counts']['vertex_probes_within_1e_minus_6'],10)
        for row in result['results']:
            if 'native_undo_history'in row:self.assertIn('Nothing to undo',row['native_undo_history']);self.assertTrue(row['local_undo_removes_outputs']);self.assertTrue(row['local_redo_restores_outputs'])
    def test_changed_attributes_and_source_selection_are_detected(self):
        q=read('fixtures/extract_curvature_graph.json');local=read('observations/extract_curvature_graph_viboceros_20261016.json');native=read('observations/extract_curvature_graph_rhino_20261016.json')
        changed=copy.deepcopy(local);changed['results'][0]['value']['outputs'][0]['selected']=True;changed['results'][0]['value']['sources_selected']=[False]
        result=comparison(q,changed,native);self.assertEqual(result['counts']['attributes_match'],9);self.assertEqual(result['counts']['source_selection_matches'],9)
