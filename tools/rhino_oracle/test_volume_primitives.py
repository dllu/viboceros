"""Independent exact flux identities, not fitted Rhino centroid targets."""
from fractions import Fraction as F
import copy
import hashlib
import json
from pathlib import Path
import unittest
from .references.volume_primitives_integrals import Polynomial, collection, reference, surface_integrals
from .references.volume_surface_primitives import patch, request
from .area_centroid_replay import prepare


class VolumePrimitiveTests(unittest.TestCase):
    def test_retained_commands_and_collection_apis_match_independent_primitives(self):
        root = Path(__file__).resolve().parent
        observed = json.loads((root/'observations/volume_surface_primitives.json').read_text())
        data = request(); refs = reference()['cases']
        native,_ = prepare(data,observed,measure='volume')
        self.assertEqual(native,data)
        for row in observed['results']:
            value = row['value']; target = refs[row['id']]['coordinate']['rounded_centroid']
            self.assertTrue(value['succeeded'])
            self.assertIsNotNone(value['open_confirmation']['dialog'])
            point = value['points'][0]['point']
            for actual in (point,value['collection']['properties']['centroid']):
                self.assertLessEqual(max(abs(x-y) for x,y in zip(actual,target)),1e-9,row['id'])
        altered = copy.deepcopy(observed)
        for row in altered['results']: row['value']['points'][0]['point'] = [123.,456.,789.]
        self.assertEqual(prepare(data,altered,measure='volume')[0],native)

    def test_capture_provenance_and_failed_attempt_are_not_rewritten(self):
        root = Path(__file__).resolve().parents[2]
        meta = json.loads((root/'docs/volume-surface-primitives-provenance.json').read_text())
        for path,digest in meta['retained_file_sha256'].items():
            self.assertEqual(hashlib.sha256((root/path).read_bytes()).hexdigest(),digest)
        failure = (root/'tools/rhino_oracle/observations/volume_surface_primitives_attempt.txt').read_text()
        self.assertIn('System.AccessViolationException',failure)
        self.assertIn('No response JSON was published',failure)
        with self.assertRaises(ValueError): json.loads(failure)
        report = json.loads((root/'docs/volume-surface-primitives-comparison.json').read_text())
        self.assertTrue(report['passed'])
        self.assertEqual(len(report['operations']),meta['completed_cases'])
        self.assertEqual(sum(row['passed'] for row in report['operations']),meta['native_command_matches'])
    def test_source_and_rational_witnesses_regenerate_without_observations(self):
        root = Path(__file__).resolve().parent/'fixtures'
        self.assertEqual(json.loads((root/'volume_surface_primitives.json').read_text()),request())
        self.assertEqual(json.loads((root/'volume_surface_primitives_reference.json').read_text()),reference())

    def test_bilinear_coordinate_primitive_and_cone_moments(self):
        s = patch([[0,0,0],[4,0,0],[0,3,0],[4,3,2]])
        base = [F(2),F(3,2),F(1)]
        self.assertEqual(surface_integrals(s,base,'cone'),(F(-2),[F(-4),F(-3),F(-4,3)]))
        self.assertEqual(surface_integrals(s,base,'coordinate'),(F(-2),[F(-10,3),F(-5,2),F(-2)]))
        self.assertEqual(surface_integrals(s,[F(2),F(3,2),F(1,2)],'coordinate'),
                         (F(0),[F(2,3),F(1,2),F(-1,2)]))

    def test_symbolic_arithmetic_and_derivatives(self):
        u = Polynomial({(1,0):1}); v = Polynomial({(0,1):1})
        p = (u+2*v)*(u-v)
        self.assertEqual(p.terms,{(2,0):F(1),(1,1):F(1),(0,2):F(-2)})
        self.assertEqual(p.derivative(0).terms,{(1,0):F(2),(0,1):F(1)})
        self.assertEqual(p.derivative(1).terms,{(1,0):F(1),(0,1):F(-4)})
        self.assertEqual(p.integral(),F(-1,12))

    def test_closed_uniform_boundaries_agree_but_mixed_gauges_are_not_physical(self):
        values = reference()['cases']
        self.assertEqual(len(values),36)
        for name in ('box-all-surfaces','box-all-meshes'):
            for mode in ('cone','coordinate'):
                self.assertEqual(values[name][mode]['volume'],'24')
                self.assertEqual(values[name][mode]['centroid'],['2','3/2','1'])
        self.assertEqual(values['box-mesh-face-0']['cone']['centroid'],['2','3/2','1'])
        self.assertEqual(values['box-mesh-face-0']['coordinate']['centroid'],['2','3/2','23/24'])
        self.assertEqual(values['two-surfaces'],values['two-surfaces-reverse-order'])

    def test_reference_changes_preserve_cancelling_anchor_distributions(self):
        for op in request()['operations']:
            if '-reference-' not in op['id']: continue
            for mode in ('cone','coordinate'):
                result = collection(op['sources'],mode)
                base = list(map(F,result['base']))
                v,m = surface_integrals(op['sources'][0],base,mode)
                self.assertEqual(str(v),result['volume'])
                self.assertEqual(list(map(str,m)),result['first'])


if __name__=='__main__': unittest.main()
