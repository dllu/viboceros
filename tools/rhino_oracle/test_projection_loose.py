"""Analytic edit-point constraints and original basis retention for Loose."""
import json
import math
from pathlib import Path
import unittest
from tools.verify_provenance import verify

ROOT=Path(__file__).resolve().parents[2]
def read(path):return json.loads((ROOT/path).read_text())


class LooseProjectionTests(unittest.TestCase):
    def test_native_and_portable_outputs_keep_original_bases_and_requested_stations(self):
        request=read('tools/rhino_oracle/fixtures/projection_loose.json');tolerance=request['tolerance']['absolute']
        for backend in ('portable','native'):
            rows=read('tools/rhino_oracle/observations/projection_loose_viboceros_'+backend+'.json')['results']
            self.assertEqual([r['id']for r in rows],[o['id']for o in request['operations']])
            for operation,row in zip(request['operations'],rows):
                with self.subTest(backend=backend,id=row['id']):
                    curves=row['value']['curves']
                    if row['id'].endswith('miss'):self.assertEqual(curves,[]);continue
                    self.assertEqual(len(curves),1)
                    curve=curves[0];definition=curve['definition'];source=operation['curve']
                    self.assertEqual(definition['degree'],source['degree']);self.assertEqual(definition['knots'],source['knots'])
                    self.assertEqual(len(definition['control_points']),len(source['control_points']))
                    self.assertEqual([c['weight']for c in definition['control_points']],[c['weight']for c in source['control_points']])
                    if row['id']=='pull-mesh-fold':
                        expected=[[-1.,0.,0.],[0.,0.,0.],[13./54.,0.,-13./54.],[1.,0.,-1.]]
                        for t,target in zip([0.,1./3.,2./3.,1.],expected):
                            point=[sum(math.comb(3,i)*(1.-t)**(3-i)*t**i*c['point'][axis]for i,c in enumerate(definition['control_points']))for axis in range(3)]
                            self.assertLessEqual(math.dist(point,target),tolerance)
                    else:
                        for actual,original in zip(definition['control_points'],source['control_points']):
                            self.assertLessEqual(math.dist(actual['point'],original['point'][:2]+[0.]),tolerance)
                        self.assertEqual(curve['closed'],row['id'].endswith('periodic'))

    def test_qualification_retains_sources_and_unqualified_native_semantics(self):
        record=read('docs/projection-loose-qualification.json')
        self.assertTrue(record['project_loose_implemented']);self.assertTrue(record['pull_loose_implemented'])
        self.assertFalse(record['full_rhino_parity']);self.assertFalse(record['performance_qualified'])
        self.assertEqual(record['fresh_rhino_observations'],0);self.assertGreaterEqual(verify(record),20)
