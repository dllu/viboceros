import copy
import json
from pathlib import Path
from unittest import TestCase
from .flow_probe import validate
from .flow_spatial_report import report
BASE=Path(__file__).parent


class FlowSpatialTests(TestCase):
    def read(self,path):return json.loads((BASE/path).read_text())
    def inputs(self):return self.read('fixtures/flow_length_frames.json'),self.read('observations/flow_length_frames_rhino_20261016.json')

    def test_bounded_public_length_frame_queries(self):
        q,_=self.inputs()
        for op in q['operations']:validate(op)
        for count in (2,129):
            op=copy.deepcopy(q['operations'][0]);op['diagnostic_length_samples']=count;validate(op)
        for count in (0,1,130,-1,True,101.,'101'):
            op=copy.deepcopy(q['operations'][0]);op['diagnostic_length_samples']=count
            with self.assertRaises(ValueError):validate(op)
        op=copy.deepcopy(q['operations'][0]);op['diagnostics']=False
        with self.assertRaises(ValueError):validate(op)

    def test_bounded_batch_length_queries(self):
        q,_=self.inputs();op=copy.deepcopy(q['operations'][0]);op['diagnostic_length_batches']=True
        for counts in ([1],[840],[4096],[100,840]):
            op['diagnostic_length_batch_divisions']=counts;validate(op)
        for counts in ([],[0],[4097],[1,1],[True],[840.],[[840]],{},list(range(1,10))):
            op['diagnostic_length_batch_divisions']=counts
            with self.assertRaises(ValueError,msg=str(counts)):validate(op)
        op['diagnostic_length_batch_divisions']=[840];op['diagnostic_length_batches']=False
        with self.assertRaises(ValueError):validate(op)
        del op['diagnostic_length_batch_divisions'];op['diagnostic_length_batches']=1
        with self.assertRaises(ValueError):validate(op)

    def test_precise_length_and_seed_aligned_public_frames_do_not_reproduce_flow(self):
        q,n=self.inputs();r=report(q,n)
        self.assertEqual(r,self.read('observations/flow_spatial_report_20261016.json'))
        self.assertEqual(r['cases'],6);self.assertEqual(r['dense']['samples'],101)
        self.assertFalse(r['native_spatial_parity_qualified']);self.assertFalse(r['native_outputs_used_as_kernel_inputs'])
        self.assertGreater(r['dense']['max_center_difference_from_precise_normalized_length'],1e-4)
        self.assertLess(r['dense']['max_center_difference_from_precise_normalized_length'],2e-4)
        for v in r['dense']['public_frame_comparisons'].values():
            self.assertGreater(v['max_roll_after_initial_alignment_degrees'],15)
            self.assertLess(v['max_roll_after_initial_alignment_degrees'],16)
        for v in r['dense']['public_fractional_length_comparisons']:self.assertGreater(v['max_coordinate_error'],1e-6)
        for v in r['initial_scale_frame_candidates']:self.assertLess(v['max_initial_basis_error'],1e-14)

    def test_diagnostic_report_rejects_incomplete_or_corrupted_geometry(self):
        q,n=self.inputs()
        for field in ('identity','row_count','parameter','origin','axis','nonfinite_point','nonfinite_map','native_basis'):
            bad=copy.deepcopy(n);v=bad['results'][0]['value'];d=v['diagnostics']['target'];r=d['length_rows'][0]
            if field=='identity':bad['results'][0]['id']='missing'
            elif field=='row_count':d['length_batch'].pop()
            elif field=='parameter':d['length_rows'][1]['t']=r['t']
            elif field=='origin':r['perp']['origin'][0]+=.1
            elif field=='axis':r['perp']['x'][0]+=.1
            elif field=='nonfinite_point':r['point'][0]=float('nan')
            elif field=='nonfinite_map':v['sdk_points'][0][0]=float('inf')
            else:v['sdk_points'][1]=[2*x for x in v['sdk_points'][1]]
            with self.assertRaises(ValueError,msg=field):report(q,bad)

    def test_scale_candidate_records_disagreement_instead_of_hiding_it(self):
        q,n=self.inputs();i=next(i for i,o in enumerate(q['operations'])if o['id']=='scale-0-1')
        plane=next(r['plane']for r in n['results'][i]['value']['diagnostics']['target']['tolerance_classification']if r['tolerance']==.1)
        plane['z']=[0,0,1]
        r=report(q,n)
        self.assertGreater(r['initial_scale_frame_candidates'][1]['max_initial_basis_error'],.1)
        self.assertFalse(r['native_spatial_parity_qualified'])

    def test_fresh_sdk_maps_reproduce_prior_captures_without_numeric_edits(self):
        q,n=self.inputs();old={}
        for name in ('flow_frames','flow_equivalence','flow_precision'):
            old.update((r['id'],r['value']['sdk_points'])for r in self.read('observations/'+name+'_rhino_20261016.json')['results'])
        for op,r in zip(q['operations'],n['results']):
            self.assertEqual(op['id'],r['id']);label='space-dense'if op['id']=='spatial'else op['id']
            self.assertEqual(r['value']['sdk_points'],old[label])
