"""Shared Dir operands, immutable producer archives and strict state comparison."""
import copy
import gzip
import hashlib
import json
import unittest
from pathlib import Path
from .brep_direction_probe import validate
from .brep_direction_comparison import canonical
from .generate_brep_direction import request
from .client import OracleProtocolError, compare_responses

HERE=Path(__file__).resolve().parent

def capture(sdk):
    family='brep_direction_sdk' if sdk else 'brep_direction_command'
    return json.loads(gzip.decompress((HERE/'observations'/(family+'.json.gz')).read_bytes()))

class BrepDirectionTests(unittest.TestCase):
    def test_shared_operands_requests_and_archived_producers_are_complete(self):
        for sdk,count in [(False,52),(True,30)]:
            family='brep_direction_sdk' if sdk else 'brep_direction_command'
            query=HERE/'fixtures'/(family+'.json');q=json.loads(query.read_text());data=capture(sdk)
            self.assertEqual(q,request(sdk));self.assertEqual(len(data['results']),count)
            self.assertEqual(data['provenance']['request_sha256'],hashlib.sha256(query.read_bytes()).hexdigest())
            for path,sha in data['provenance']['sources'].items():
                archived=gzip.decompress((HERE/'observations/source_snapshots'/(sha+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(archived).hexdigest(),sha,path)
                if path.endswith('.3dm'):self.assertEqual((HERE/path).read_bytes(),archived)
            self.assertEqual([r['id'] for r in data['results']],[op['id'] for op in q['operations']])
            for op,row in zip(q['operations'],data['results']):canonical(op,row['value'],True)

    def test_selection_and_undo_keep_the_two_native_execution_routes_distinct(self):
        for sdk in (False,True):
            for op,row in zip(request(sdk)['operations'],capture(sdk)['results']):
                values=[row['value'][phase]['selected'] for phase in ('before','after','undo','redo')]
                self.assertEqual(values,[True]*4 if sdk else [op['preselect'],op['preselect'],False,op['preselect']])
                self.assertEqual(row['value']['before']['geometry'],row['value']['undo']['geometry'])
                self.assertEqual(row['value']['after']['geometry'],row['value']['redo']['geometry'])

    def test_public_comparison_keeps_geometry_selection_metadata_and_history(self):
        for sdk in (False,True):
            q=request(sdk);native=capture(sdk);local=copy.deepcopy(native)
            local['engine']='viboceros';local.pop('provenance')
            for op,row in zip(q['operations'],local['results']):row['value']=canonical(op,row['value'],True)
            self.assertTrue(compare_responses(local,native,absolute_epsilon=1e-9,relative_epsilon=0.,request=q).passed)
            for phase in ('before','after','undo','redo'):
                broken=copy.deepcopy(local);broken['results'][0]['value'][phase]['selected']^=True
                self.assertFalse(compare_responses(broken,native,absolute_epsilon=1e-9,relative_epsilon=0.,request=q).passed)
            broken=copy.deepcopy(local)
            broken['results'][0]['value']['after']['geometry']['faces'][0]['samples'][0][0]+=.01
            self.assertFalse(compare_responses(broken,native,absolute_epsilon=1e-9,relative_epsilon=0.,request=q).passed)

    def test_malformed_inputs_invalid_traces_and_nonfinite_geometry_are_rejected(self):
        op=request()['operations'][0];v=capture(False)['results'][0]['value']
        for key,val in [('source','../source'),('edit','_Delete'),('preselect',1),('faces',[True]),('faces',[0,0]),('macro','arbitrary')]:
            broken=dict(op,**{key:val})
            with self.subTest(key=key),self.assertRaises(ValueError):validate(broken)
        for change in ('events','success','nan','count'):
            broken=copy.deepcopy(v)
            if change=='events':broken['events']*=2
            elif change=='success':broken['success']=False
            elif change=='nan':broken['after']['geometry']['vertices'][0]['point'][0]=float('nan')
            else:broken['after']['geometry']['topology']['vertices']=True
            with self.subTest(change=change),self.assertRaises(OracleProtocolError):canonical(op,broken,True)

if __name__=='__main__':unittest.main()
