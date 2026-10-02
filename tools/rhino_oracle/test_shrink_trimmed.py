import copy
import json
import unittest
from pathlib import Path
from .client import OracleProtocolError, _owned_artifact_request
from .shrink_trimmed_probe import validate
from .shrink_trimmed_cases import request, extended_request, geometry_request
from .shrink_trimmed_capture import validate_request
from .shrink_trimmed_replay import canonical_response


class ShrinkTrimmedTests(unittest.TestCase):
    def test_saved_sources_are_generated_without_native_outputs(self):
        for name,make in [('shrink_trimmed_surfaces',request),('shrink_trimmed_history',extended_request),('shrink_trimmed_geometry',geometry_request)]:
            saved=json.loads(Path(__file__).with_name('fixtures').joinpath(name+'.json').read_text())
            self.assertEqual(saved,make())
            with _owned_artifact_request(saved) as owned:
                validate_request(owned)
                self.assertTrue(all('artifact_path' in s['brep'] for op in owned['operations'] for s in op['sources']))
            self.assertNotIn('artifact_path',json.dumps(saved))

    def test_rejects_invalid_selection_orders_and_ambiguous_runs(self):
        with _owned_artifact_request(request()) as owned:
            op=owned['operations'][0]
            for key,value in [('preselect',1),('undo_redo',0),('order',[True]),('order',[]),('order',[0,0]),('order',[1]),('sources',[]),('id','unsafe path')]:
                invalid=copy.deepcopy(op);invalid[key]=value
                with self.subTest(key=key,value=value),self.assertRaises(ValueError):validate(invalid)
            invalid=copy.deepcopy(owned);invalid['iterations']=2
            with self.assertRaises(ValueError):validate_request(invalid)
            invalid=copy.deepcopy(owned);invalid['operations'][1]['id']=invalid['operations'][0]['id']
            with self.assertRaises(ValueError):validate_request(invalid)

    def test_identity_sort_does_not_normalize_geometry_or_numeric_indices(self):
        req=dict(operations=[dict(id='case',sources=[{},{}])])
        response=dict(protocol_version=1,iterations=1,results=[dict(id='case',value=dict(after=[dict(source=1,geometry=dict(edges=[3,0])),dict(source=0,geometry=dict(edges=[2,1]))],history='diagnostic'))])
        result=canonical_response(req,response)
        self.assertEqual(result['results'][0]['value']['after'],[dict(source=0,geometry=dict(edges=[2,1])),dict(source=1,geometry=dict(edges=[3,0]))])
        self.assertEqual(response['results'][0]['value']['after'][0]['source'],1)
        malformed=copy.deepcopy(response);malformed['results'][0]['value']['after'][0]['source']=0
        with self.assertRaises(OracleProtocolError):canonical_response(req,malformed)


if __name__=='__main__':unittest.main()
