"""Frozen public analysis evidence and strict comparison contracts."""
import copy
import gzip
import hashlib
import json
import unittest
from .client import OracleProtocolError,compare_responses
from .draft_angle_comparison import canonical
from .draft_angle_probe import validate
from .generate_draft_angle import HERE,FAMILY,request

def load():
    query=HERE/'fixtures'/(FAMILY+'.json')
    return query,json.loads(query.read_text()),json.loads(gzip.decompress((HERE/'observations'/(FAMILY+'.json.gz')).read_bytes()))

class DraftAngleTests(unittest.TestCase):
    def test_native_capture_and_archived_producers_are_complete(self):
        query,q,n=load();self.assertEqual(q,request());self.assertEqual(len(n['results']),22)
        self.assertEqual(n['provenance']['request_sha256'],hashlib.sha256(query.read_bytes()).hexdigest())
        for path,sha in n['provenance']['sources'].items():
            raw=gzip.decompress((HERE/'observations/source_snapshots'/(sha+'.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(raw).hexdigest(),sha,path)
            if path.endswith('.3dm'):self.assertEqual(raw,(HERE/path).read_bytes())
        samples=0
        for op,row in zip(q['operations'],n['results']):
            self.assertEqual(op['id'],row['id']);canonical(op,row['value'],True)
            samples+=sum(len(m['colors']) for t in row['value']['traces'] for m in t['meshes'])
        self.assertEqual(samples,10160)

    def test_comparison_preserves_membership_selection_visibility_and_settings(self):
        _,q,n=load();local=copy.deepcopy(n);local['engine']='viboceros';local.pop('provenance')
        for op,row in zip(q['operations'],local['results']):row['value']=canonical(op,row['value'],True)
        self.assertTrue(compare_responses(local,n,absolute_epsilon=1e-9,relative_epsilon=0,request=q).passed)
        for field in ('selected','hidden','analysis'):
            b=copy.deepcopy(local);b['results'][0]['value']['states'][1][field]^=True
            self.assertFalse(compare_responses(b,n,absolute_epsilon=1e-9,relative_epsilon=0,request=q).passed)
        for field in ('range','up','wires'):
            b=copy.deepcopy(local);s=b['results'][0]['value']['settings'][1]
            if field=='wires':s[field]^=True
            else:s[field][0]+=.01
            self.assertFalse(compare_responses(b,n,absolute_epsilon=1e-9,relative_epsilon=0,request=q).passed)

    def test_invalid_recipes_and_malformed_native_evidence_are_rejected(self):
        _,q,n=load();op=q['operations'][2];v=n['results'][2]['value']
        for change in ({'source':'../bad'},{'range':[0,float('nan')]},{'range':[0,91]},{'up':[0,0,0]},{'preselect':1},{'wires':1},{'steps':[]},{'steps':['DeleteAll']}):
            with self.subTest(change=change),self.assertRaises(ValueError):validate(dict(op,**change))
        for field in ('states','trace','colors','normals','indices','nonfinite'):
            b=copy.deepcopy(v)
            if field=='states':b['states'].pop()
            elif field=='trace':b['traces'][0]['events']*=2
            else:
                m=b['traces'][0]['meshes'][0]
                if field=='colors':m['colors'][0][0]=256
                elif field=='normals':m['normals'][0]=[0,0,0]
                elif field=='indices':m['indices'][0]=m['vertex_count']
                else:m['normals'][0][0]=float('nan')
            with self.subTest(field=field),self.assertRaises(OracleProtocolError):canonical(op,b,True)

if __name__=='__main__':unittest.main()
