"""Frozen public endpoint precision and ordered tangent batches."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import unittest
from .curve_tangents_probe import validate
from .generate_curve_tangents import request as tangents
from .generate_loft_refit_endpoint_precision import request as endpoints
from .generate_loft_refit_candidate_precision import request as candidates
from .generate_curve_tangents_stationary import request as stationary
from .generate_curve_tangents_domains import request as domains

HERE=Path(__file__).resolve().parent


class CurveTangentTests(unittest.TestCase):
    def test_frozen_native_requests_and_producers(self):
        for family,build,count in [('curve_tangents',tangents,14),
                ('loft_refit_endpoint_precision',endpoints,7),
                ('curve_tangents_stationary',stationary,2),
                ('curve_tangents_domains',domains,2),
                ('loft_refit_candidate_precision',candidates,14)]:
            with self.subTest(family=family):
                encoded=(HERE/('fixtures/'+family+'.json')).read_bytes()
                data=json.loads((HERE/('observations/'+family+'.json')).read_text())
                self.assertEqual(json.loads(encoded),build());self.assertEqual(len(data['results']),count)
                self.assertEqual(data['engine'],'rhino')
                self.assertEqual(data['provenance']['request_sha256'],hashlib.sha256(encoded).hexdigest())
                self.assertEqual([r['id']for r in data['results']],[op['id']for op in build()['operations']])
                for digest in data['provenance']['sources'].values():
                    archived=gzip.decompress((HERE/('observations/source_snapshots/'+digest+'.gz')).read_bytes())
                    self.assertEqual(hashlib.sha256(archived).hexdigest(),digest)

    def test_native_tangents_retain_order_inputs_and_hard_corner_policy(self):
        data=json.loads((HERE/'observations/curve_tangents.json').read_text())
        for op,row in zip(tangents()['operations'],data['results']):
            validate(op);value=row['value'];self.assertEqual(value['before'],value['after'])
            self.assertEqual(value['parameters'],op['parameters']);self.assertEqual(len(value['tangents']),4)
            self.assertEqual(value['tangents'][0],value['tangents'][3])
        polyline=data['results'][-1]['value']['tangents']
        self.assertEqual(polyline,[[0.,1.,0.],[1.,0.,0.],[0.,1.,0.],[0.,1.,0.]])

    def test_invalid_query_metadata_and_resource_counts_are_rejected(self):
        original=tangents()['operations'][0]
        for values in [[],[-1.],[2.],[float('nan')],[float('inf')],[True],[0.]*4097]:
            op=copy.deepcopy(original);op['parameters']=values
            with self.subTest(parameters=values[:2]):
                with self.assertRaises(ValueError):validate(op)
        for domain in [[],[0.],[0.,0.],[1.,0.],[float('nan'),1.],[0.,float('inf')]]:
            op=copy.deepcopy(original);op['curve']['domain']=domain
            with self.assertRaises(ValueError):validate(op)
        op=copy.deepcopy(original);op['curve']['domain']=None;op['parameters']=[0.]*4096
        validate(op)

    def test_storage_flag_does_not_change_these_candidate_directions(self):
        data=json.loads((HERE/'observations/loft_refit_candidate_precision.json').read_text())
        for a,b in zip(data['results'][::2],data['results'][1::2]):
            a,b=a['value']['refit_adjustment'],b['value']['refit_adjustment']
            self.assertTrue(a['candidate_is_rational']);self.assertFalse(b['candidate_is_rational'])
            self.assertEqual(a['candidate_endpoint_tangents'],b['candidate_endpoint_tangents'])
        center=data['results'][2]['value']['refit_adjustment']
        self.assertGreater(center['candidate_endpoint_tangent_dot'],-.9999)

    def test_stationary_availability_and_explicit_domains(self):
        data=json.loads((HERE/'observations/curve_tangents_stationary.json').read_text())
        self.assertEqual([row['value']['tangents'].count(None)for row in data['results']],[1,2])
        for op in domains()['operations']:
            validate(op)
            bad=copy.deepcopy(op);bad['parameters']=[op['curve']['domain'][1]+1.]
            with self.assertRaises(ValueError):validate(bad)
