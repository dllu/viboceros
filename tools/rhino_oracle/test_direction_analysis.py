"""Immutable ShowDir UI captures and strict model/display comparison contracts."""
import copy
import gzip
import hashlib
import json
import unittest
from .client import OracleProtocolError,compare_responses
from .direction_analysis_comparison import canonical
from .direction_analysis_probe import validate
from .generate_direction_analysis import HERE,request,controls_request,membership_request,foreign_request,multi_request

FAMILIES=[('direction_analysis_initial',request,12),('direction_analysis_controls_v2',controls_request,30),
          ('direction_analysis_membership_v3',membership_request,2),('direction_analysis_foreign',foreign_request,4)]
FAMILIES.append(('direction_analysis_multi',multi_request,3))

def load(family):
    query=HERE/'fixtures'/(family+'.json');q=json.loads(query.read_text())
    data=json.loads(gzip.decompress((HERE/'observations'/(family+'.json.gz')).read_bytes()))
    return query,q,data

class DirectionAnalysisTests(unittest.TestCase):
    def test_requests_shared_operands_and_frozen_producers_are_complete(self):
        for family,factory,count in FAMILIES:
            query,q,data=load(family);self.assertEqual(q,factory());self.assertEqual(len(data['results']),count)
            self.assertEqual(data['provenance']['request_sha256'],hashlib.sha256(query.read_bytes()).hexdigest())
            for path,sha in data['provenance']['sources'].items():
                archived=gzip.decompress((HERE/'observations/source_snapshots'/(sha+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(archived).hexdigest(),sha,path)
                if path.endswith('.3dm'):self.assertEqual(archived,(HERE/path).read_bytes())
            for op,row in zip(q['operations'],data['results']):
                self.assertEqual(op['id'],row['id']);canonical(op,row['value'],True)

    def test_public_comparison_keeps_each_source_selection_geometry_membership_and_flags(self):
        for family,_,_ in FAMILIES:
            _,q,native=load(family);local=copy.deepcopy(native);local['engine']='viboceros';local.pop('provenance')
            for op,row in zip(q['operations'],local['results']):row['value']=canonical(op,row['value'],True)
            self.assertTrue(compare_responses(local,native,absolute_epsilon=1e-9,relative_epsilon=0.,request=q).passed)
            for field in ('selected','analysis','hidden'):
                broken=copy.deepcopy(local);state=broken['results'][0]['value']['states'][1]
                if q['operations'][0].get('companion'):state=state[1]
                state[field]^=True
                self.assertFalse(compare_responses(broken,native,absolute_epsilon=1e-9,relative_epsilon=0.,request=q).passed)
            broken=copy.deepcopy(local);broken['results'][0]['value']['settings'][1]['direction']^=True
            self.assertFalse(compare_responses(broken,native,absolute_epsilon=1e-9,relative_epsilon=0.,request=q).passed)
            broken=copy.deepcopy(local);state=broken['results'][0]['value']['states'][1]
            if q['operations'][0].get('companion'):state=state[1]
            state['geometry']['vertices'][0]['point'][0]+=.01
            self.assertFalse(compare_responses(broken,native,absolute_epsilon=1e-9,relative_epsilon=0.,request=q).passed)

    def test_malformed_operations_nonfinite_geometry_and_spoofed_controls_are_rejected(self):
        _,q,data=load('direction_analysis_initial');op=q['operations'][0];value=data['results'][0]['value']
        for extra in ({'source':'../file'},{'steps':['_Delete']},{'steps':[]},{'preselect':1},{'companion':1},{'picks':{'0':99}}):
            with self.subTest(extra=extra),self.assertRaises(ValueError):validate(dict(op,**extra))
        for field in ('count','events','checkbox','nan','mode'):
            broken=copy.deepcopy(value)
            if field=='count':broken['states'].pop()
            elif field=='events':broken['traces'][0]['events']*=2
            elif field=='checkbox':
                control=next(r for r in broken['traces'][0]['controls'] if r['label']=='Direction');control['check']=2
            elif field=='mode':broken['traces'][0]['modes'][0]['name']='Different analysis'
            else:broken['states'][1]['geometry']['vertices'][0]['point'][0]=float('nan')
            with self.subTest(field=field),self.assertRaises(OracleProtocolError):canonical(op,broken,True)

if __name__=='__main__':unittest.main()
