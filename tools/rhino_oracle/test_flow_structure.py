import json
from pathlib import Path
from unittest import TestCase,skipIf
try:
    import numpy as np
except ModuleNotFoundError:
    np=None
if np is not None:
    from .flow_structure_report import report,fixed_direction
BASE=Path(__file__).parent


@skipIf(np is None,'optional Flow structure analysis requires NumPy')
class FlowStructureTests(TestCase):
    def test_retained_structure_report_and_scope_are_reproducible(self):
        actual=report();saved=json.loads((BASE/'observations/flow_structure_report_20261016.json').read_text())
        def close(a,b):
            if isinstance(a,dict):return a.keys()==b.keys()and all(close(a[k],b[k])for k in a)
            if isinstance(a,list):return len(a)==len(b)and all(close(x,y)for x,y in zip(a,b))
            if isinstance(a,float):return abs(a-b)<=1e-10
            return a==b
        self.assertTrue(close(actual,saved));self.assertEqual(actual['cases'],13)
        self.assertFalse(actual['production_kernel_changed']);self.assertFalse(actual['spatial_flow_qualified']);self.assertFalse(actual['native_values_used_as_kernel_inputs'])

    def test_public_840_lookup_predicts_held_out_flow_centers_and_degree_elevation(self):
        r=report();lookups=r['lookup']['comparisons'];self.assertEqual([v['samples']for v in lookups],[2048,682,2048])
        for v in lookups:self.assertLess(v['max_coordinate_error'],1e-12)
        for v in r['batch_100_comparisons']:self.assertGreater(v['max_coordinate_error'],1e-6)
        for v in r['guide_representation_comparisons']:self.assertLess(v['max_coordinate_error'],1e-12)

    def test_fixed_direction_candidates_predict_unfitted_sections_but_vary_by_shape(self):
        rows=report()['fixed_direction_candidates']
        for r in rows:
            self.assertGreater(r['checked_sections'],r['training_sections'])
            self.assertLess(r['max_shared_direction_basis_error'],1e-11)
            self.assertLess(r['cross_coefficient_parallel_error'],1e-10)
            self.assertAlmostEqual(np.linalg.norm(r['fitted_direction']),1.,places=12)
        self.assertGreater(np.linalg.norm(np.array(rows[0]['fitted_direction'])-rows[2]['fitted_direction']),.1)

    def test_invalid_frames_fail_and_valid_rotational_corruption_remains_visible(self):
        native=json.loads((BASE/'observations/flow_density_rhino_20261016.json').read_text());p=np.array(native['results'][1]['value']['sdk_points'])
        bad=p.copy();bad[1]*=2
        with self.assertRaises(ValueError):fixed_direction(bad)
        bad=p.copy();bad[1,0]=float('nan')
        with self.assertRaises(ValueError):fixed_direction(bad)
        bad=p.copy();i=3*101;y,z=bad[i+1]-bad[i],bad[i+2]-bad[i];angle=.1
        bad[i+1]=bad[i]+np.cos(angle)*y+np.sin(angle)*z;bad[i+2]=bad[i]-np.sin(angle)*y+np.cos(angle)*z
        self.assertGreater(fixed_direction(bad)['max_shared_direction_basis_error'],.01)
