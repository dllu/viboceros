"""Reference-only source-distance readback selection and identity checks."""
import copy
from unittest import TestCase
from .network_surface_probe import request
from .network_surface_readback import source_distances_from_observations


class NetworkSourceReadbackTests(TestCase):
    def inputs(self):
        specification=request();specification['operations']=specification['operations'][:1]
        op=specification['operations'][0];op['source_sample_fractions']=[0.,.5,1.]
        definition=dict(degree=[1,1],control_count=[2,2],control_points=[
            dict(point=p,weight=1.)for p in ([0,0,0],[1,0,0],[0,1,0],[1,1,0])],
            knots_u=[-3.,-3.,7.,7.],knots_v=[100.,100.,200.,200.],
            domain_u=[-3.,7.],domain_v=[100.,200.])
        samples=[dict(source_curve_index=i,source_fraction=f,source_point=[i,f,0.],distance=d)
            for i in range(len(op['curves']))for f,d in zip(op['source_sample_fractions'],[0.,4.,1.])]
        observations=dict(engine='rhino',results=[dict(id=op['id'],value=dict(definition=definition,source_samples=samples))])
        return specification,observations

    def test_selects_one_worst_source_query_per_curve_without_changing_reference_basis(self):
        specification,observations=self.inputs();before=copy.deepcopy((specification,observations))
        result=source_distances_from_observations(specification,observations)
        op=result['operations'][0]
        self.assertEqual(op['points'],[[i,.5,0.]for i in range(4)])
        self.assertEqual(op['surface']['domain_u'],[-3.,7.]);self.assertEqual(op['surface']['domain_v'],[100.,200.])
        self.assertEqual(op['surface']['knots_u'],[-3.,-3.,7.,7.])
        self.assertEqual(op['surface']['control_points'],observations['results'][0]['value']['definition']['control_points'])
        self.assertEqual((specification,observations),before)

    def test_rejects_mismatched_counts_samples_and_nonfinite_distances(self):
        for failure in ['engine','count','id','missing','curve','fraction','nan','negative']:
            specification,observations=self.inputs();sample=observations['results'][0]['value']['source_samples'][1]
            if failure=='engine':observations['engine']='viboceros'
            elif failure=='count':observations['results']=[]
            elif failure=='id':observations['results'][0]['id']='other'
            elif failure=='missing':observations['results'][0]['value']['source_samples'].pop()
            elif failure=='curve':sample['source_curve_index']=1
            elif failure=='fraction':sample['source_fraction']=.25
            elif failure=='nan':sample['distance']=float('nan')
            else:sample['distance']=-1.
            with self.subTest(failure=failure),self.assertRaises(ValueError):
                source_distances_from_observations(specification,observations)
