"""Native draft-direction corpus, public input receipts and immutable producers."""
import copy
import gzip
import hashlib
import json
import math
import unittest
from .draft_pull_probe import validate
from .generate_draft_pull import HERE,FAMILY,request


class DraftPullTests(unittest.TestCase):
    def test_native_controls_and_frozen_provenance(self):
        path=HERE/'fixtures'/(FAMILY+'.json');q=json.loads(path.read_text())
        n=json.loads(gzip.decompress((HERE/'observations'/(FAMILY+'.json.gz')).read_bytes()))
        self.assertEqual(q,request());self.assertEqual(len(q['operations']),26)
        self.assertEqual(n['provenance']['request_sha256'],hashlib.sha256(path.read_bytes()).hexdigest())
        for name,sha in n['provenance']['sources'].items():
            raw=gzip.decompress((HERE/'observations/source_snapshots'/(sha+'.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(raw).hexdigest(),sha,name)
            if name.endswith('.3dm'):self.assertEqual(raw,(HERE/name).read_bytes())
        self.assertEqual(len(q['operations']),len(n['results']))
        for op,row in zip(q['operations'],n['results']):
            with self.subTest(case=op['id']):
                self.assertEqual(op['id'],row['id']);self.assertNotIn('error',row)
                value=row['value'];self.assertEqual(value['choices'],['CPlane Z','World Z','View based','Two points','Curve or edge'])
                self.assertEqual(value['input']['sent'],[])
                for trace in (value['input'],value['repeated']):self.assertEqual(trace['errors'],[])
                self.assertTrue(all(type(x) in (int,float) and math.isfinite(x) for x in value['up']))
                if op['mode'] in ('TwoPoints','Curve'):self.assertEqual(value['selected'],[9,8,7])
                else:self.assertEqual(value['selected'],value['up'])
                if op['cancel']:self.assertEqual(value['up'],[9,8,7])
                if op['mode']=='TwoPoints':
                    self.assertEqual(len(value['repeated']['sent']),2)
                    if op['cancel']:self.assertEqual(value['repeated']['sent'][1]['token'],'_Cancel')
                    else:
                        expected=[b-a for a,b in zip(*op['points'])]
                        self.assertEqual(value['up'],expected)
                elif op['mode']=='Curve' and not op['cancel']:
                    self.assertAlmostEqual(sum(x*x for x in value['up']),1,places=12)
                    if op['curve']=='BoxEdge':self.assertTrue(value['repeated']['edge_pick_acknowledged'])

    def test_invalid_recipes_reject_unbounded_or_ambiguous_input(self):
        op=request()['operations'][0]
        for changes in ({'id':'../file'},{'mode':'Bad'},{'curve':'Bad'},{'plane':[0,0,0]},
                        {'view':[True,0,1]},{'points':[[0,0,0],[float('nan'),0,0]]},
                        {'reverse':1},{'cancel':1},{'edge':12},{'mode':'Curve'}):
            with self.subTest(changes=changes),self.assertRaises(ValueError):validate(dict(copy.deepcopy(op),**changes))


if __name__=='__main__':unittest.main()
