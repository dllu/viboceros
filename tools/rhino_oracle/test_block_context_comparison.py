"""Reconstruction permutation never erases attached metadata or numeric geometry."""
import copy
import unittest
from .block_context_comparison import canonical
from .client import compare_responses

class BlockContextComparisonTests(unittest.TestCase):
    def value(self):
        members=[dict(attributes=dict(name='a'),geometry=dict(kind='point',points=[[1,2,3]])),dict(attributes=dict(name='b'),geometry=dict(kind='point',points=[[4,5,6]]))]
        leaves=[dict(member,path=[['part',i]]) for i,member in enumerate(members)]
        return dict(comparison_policy='block_context_member_permutation',states=[dict(definitions=[dict(name='part',members=members)],objects=[dict(handle=0,geometry=dict(kind='block',leaves=leaves))])])
    def test_member_permutation_remaps_leaf_paths_and_preserves_raw_inputs(self):
        first=self.value();raw=copy.deepcopy(first);second=copy.deepcopy(first)
        second['states'][0]['definitions'][0]['members'].reverse()
        for leaf in second['states'][0]['objects'][0]['geometry']['leaves']:leaf['path'][0][1]=1-leaf['path'][0][1]
        second['states'][0]['objects'][0]['geometry']['leaves'].reverse()
        self.assertEqual(canonical(first),canonical(second));self.assertEqual(first,raw)
    def test_geometry_and_metadata_changes_still_compare_unequally(self):
        first=self.value()
        for field,value in [('attributes',dict(name='changed')),('geometry',dict(kind='point',points=[[9,2,3]]))]:
            second=copy.deepcopy(first);second['states'][0]['definitions'][0]['members'][0][field]=value
            self.assertNotEqual(canonical(first),canonical(second))
    def test_inconsistent_paths_are_not_hidden_by_member_sorting(self):
        first=self.value();second=copy.deepcopy(first);second['states'][0]['definitions'][0]['members'].reverse()
        self.assertNotEqual(canonical(first),canonical(second))
    def test_response_comparison_applies_only_the_explicit_context_policy(self):
        first=self.value();second=copy.deepcopy(first)
        second['states'][0]['definitions'][0]['members'].reverse()
        for leaf in second['states'][0]['objects'][0]['geometry']['leaves']:leaf['path'][0][1]=1-leaf['path'][0][1]
        def response(engine,value):return dict(protocol_version=1,engine=engine,engine_version='test',iterations=1,results=[dict(id='case',value=value,elapsed_ns=0)])
        self.assertTrue(compare_responses(response('viboceros',first),response('rhino',second)).passed)
        first.pop('comparison_policy');second.pop('comparison_policy')
        self.assertFalse(compare_responses(response('viboceros',first),response('rhino',second)).passed)
