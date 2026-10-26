"""Straightness and degree are separate inputs in the retained scalar probes."""
import gzip
import hashlib
import json
from pathlib import Path
import unittest
from .generate_scalar_linearity import request

HERE = Path(__file__).resolve().parent


class ScalarLinearityTests(unittest.TestCase):
    def test_recipe_archives_and_source_purity(self):
        encoded = (HERE/'fixtures/scalar_linearity.json').read_bytes()
        query = json.loads(encoded)
        native = json.loads((HERE/'observations/scalar_linearity.json').read_text())
        self.assertEqual(query,request())
        self.assertEqual(len(query['operations']),9)
        self.assertEqual([o['id'] for o in query['operations']],[r['id'] for r in native['results']])
        self.assertEqual(hashlib.sha256(encoded).hexdigest(),native['provenance']['request_sha256'])
        for name,digest in native['provenance']['sources'].items():
            data = gzip.decompress((HERE/('observations/source_snapshots/'+digest+'.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(data).hexdigest(),digest,name)
        for row in native['results']:
            self.assertEqual(row['value']['before'],row['value']['after'])

    def test_degree_elevation_retains_the_line_and_native_scalar_stations(self):
        query = request()
        native = json.loads((HERE/'observations/scalar_linearity.json').read_text())
        rows = {r['id']:r['value'] for r in native['results']}
        reference = rows['scalar-linearity-line-degree-1']
        for op in query['operations']:
            if '-line-degree-' not in op['id']:
                continue
            curve = op['curve']
            for i in range(17):
                t = i/16.
                net = [[x*p['weight'] for x in p['point']]+[p['weight']]
                       for p in curve['control_points']]
                while len(net)>1:
                    net = [[(1.-t)*x+t*y for x,y in zip(a,b)] for a,b in zip(net,net[1:])]
                point = [x/net[0][3] for x in net[0][:3]]
                expected = [8.*t/(1.+t),4.*t/(1.+t),0.]
                self.assertLess(max(abs(x-y) for x,y in zip(point,expected)),2e-14)
            for a,b in zip(rows[op['id']]['parameters'],reference['parameters']):
                self.assertAlmostEqual(a,b,delta=1e-12)


if __name__=='__main__':
    unittest.main()
