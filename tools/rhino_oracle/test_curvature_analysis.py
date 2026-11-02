"""Frozen curvature recipes, provenance, and public comparison failure checks."""
import copy
import gzip
import hashlib
import json
import unittest
from .client import OracleProtocolError,compare_responses
from .curvature_analysis_comparison import canonical
from .curvature_analysis_probe import validate
from .generate_curvature_analysis import HERE,FAMILY,request,ranges_request

def load(family=FAMILY):
    p=HERE/'fixtures'/(family+'.json')
    return p,json.loads(p.read_text()),json.loads(gzip.decompress((HERE/'observations'/(family+'.json.gz')).read_bytes()))

class CurvatureAnalysisTests(unittest.TestCase):
    def test_requests_and_archived_native_producers_remain_complete(self):
        for family in (FAMILY,'surface_analysis_input','curvature_analysis_ranges'):
            p,q,n=load(family)
            self.assertEqual(n['provenance']['request_sha256'],hashlib.sha256(p.read_bytes()).hexdigest())
            for path,sha in n['provenance']['sources'].items():
                raw=gzip.decompress((HERE/'observations/source_snapshots'/(sha+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(raw).hexdigest(),sha,path)
                if path.endswith('.3dm'):self.assertEqual(raw,(HERE/path).read_bytes())
            if family==FAMILY:
                self.assertEqual(q,request());self.assertEqual(len(n['results']),52);count=0
                for op,row in zip(q['operations'],n['results']):
                    self.assertEqual(op['id'],row['id']);canonical(op,row['value'],True)
                    count+=sum(len(m['samples']) for t in row['value']['traces'] for m in t['meshes'])
                self.assertEqual(count,11536)
            elif family=='curvature_analysis_ranges':
                self.assertEqual(q,ranges_request());self.assertEqual(len(n['results']),12)
                for op,row in zip(q['operations'],n['results']):canonical(op,row['value'],True)
            else:
                rows=n['results'][0]['value'];self.assertEqual([(r['source'],r['analysis']) for r in rows],[('mesh',False),('subd',True)])
                for row in rows:self.assertEqual(row['events'],[dict(name='CurvatureAnalysis',result='Success')])

    def test_public_comparison_preserves_selection_visibility_ownership_and_ranges(self):
        _,q,n=load();local=copy.deepcopy(n);local['engine']='viboceros';local.pop('provenance')
        for op,row in zip(q['operations'],local['results']):row['value']=canonical(op,row['value'],True)
        self.assertTrue(compare_responses(local,n,absolute_epsilon=1e-9,relative_epsilon=0,request=q).passed)
        for field in ('selected','hidden','analysis','draft'):
            broken=copy.deepcopy(local);broken['results'][0]['value']['states'][1][field]^=True
            if field=='draft':
                with self.assertRaises(OracleProtocolError):compare_responses(broken,n,request=q)
            else:self.assertFalse(compare_responses(broken,n,absolute_epsilon=1e-9,relative_epsilon=0,request=q).passed)
        broken=copy.deepcopy(local);broken['results'][0]['value']['settings'][1]['ranges'][0][0]+=.01
        self.assertFalse(compare_responses(broken,n,absolute_epsilon=1e-9,relative_epsilon=0,request=q).passed)

    def test_invalid_recipes_and_corrupted_native_samples_are_rejected(self):
        _,q,n=load();op=q['operations'][0];value=n['results'][0]['value']
        for changes in ({'source':'../file'},{'preselect':1},{'range':[float('nan'),1]},{'style':'Bad'},{'steps':[]},{'steps':['DeleteAll']}):
            with self.subTest(changes=changes),self.assertRaises(ValueError):validate(dict(op,**changes))
        for field in ('events','nonfinite','color','index','state'):
            broken=copy.deepcopy(value)
            if field=='events':broken['traces'][0]['events']*=2
            elif field=='state':broken['states'][1]['draft']=True
            else:
                sample=broken['traces'][0]['meshes'][0]['samples'][0]
                if field=='nonfinite':sample['principal'][0]=float('nan')
                elif field=='color':sample['color'][0]=256
                else:sample['index']=-1
            with self.subTest(field=field),self.assertRaises(OracleProtocolError):canonical(op,broken,True)

if __name__=='__main__':unittest.main()
