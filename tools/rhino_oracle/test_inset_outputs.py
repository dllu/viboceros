"""Owned Inset output geometry, metadata, option clocks and limit evidence."""
import gzip
import hashlib
import json
import unittest
from .capture_family import HERE
from .generate_inset_surface import request
from .inset_outputs_probe import cases, limits, validate, PREFERENCES


def native(family):
    return json.loads(gzip.decompress((HERE / ('observations/' + family + '.json.gz')).read_bytes()))


class InsetOutputTests(unittest.TestCase):
    def test_immutable_owned_requests_and_producers(self):
        for family,operations in [('inset_output_modes',cases()),('inset_output_limits',limits()),
                                  ('inset_output_preferences',[PREFERENCES])]:
            query=request();query['operations']=operations
            path=HERE / ('fixtures/' + family + '.json'); data=native(family)
            self.assertEqual(json.loads(path.read_bytes()),query)
            self.assertEqual([r['id'] for r in data['results']],[op['id'] for op in operations])
            self.assertEqual(data['provenance']['request_sha256'],hashlib.sha256(path.read_bytes()).hexdigest())
            for sha in data['provenance']['sources'].values():
                source=gzip.decompress((HERE / ('observations/source_snapshots/' + sha + '.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(source).hexdigest(),sha)
            for row in data['results']:self.assertNotIn('error',row)

    def test_curve_sources_survive_and_separate_surfaces_get_default_attributes(self):
        for op,row in zip(cases(),native('inset_output_modes')['results']):
            value=row['value'];self.assertEqual(value['results'],['Success'])
            self.assertNotIn('Unknown command:',value['history'])
            if op['split']:
                self.assertTrue(all(o['source'] is None and 'brep' in o for o in value['after']))
                self.assertEqual(value['before'][0]['brep'],value['undo'][0]['brep'])
            else:
                self.assertEqual(value['after'][0]['source'],0)
                self.assertEqual(value['before'][0]['brep'],value['after'][0]['brep'])
                self.assertTrue(all('curve' in o and 'definition' in o['curve'] for o in value['after'][1:]))
            for obj in value['after']:
                if obj['source'] is None:
                    self.assertIsNone(obj['name']);self.assertIsNone(obj['layer'])
                    self.assertEqual(obj['groups'],[]);self.assertEqual(obj['color'],[0,0,0])
                    self.assertFalse(obj['selected'])
            self.assertEqual(value['after_script'],value['redo'])

    def test_completed_and_canceled_output_options(self):
        steps=native('inset_output_preferences')['results'][0]['value']['steps']
        self.assertEqual(len(steps),12)
        split=join=True
        for step in steps:
            value=step['value'];op=step['input'];first=value['history'].splitlines()[1]
            self.assertIn('Split='+('Yes' if split else 'No'),first)
            if split:self.assertIn('JoinOutputs='+('Yes' if join else 'No'),first)
            else:self.assertNotIn('JoinOutputs=',first)
            self.assertEqual(value['results'],['Cancel' if op['cancel'] else 'Success'])
            if not op['cancel']:
                if op['split'] is not None:split=op['split']
                if op['join_outputs'] is not None:join=op['join_outputs']

    def test_limit_outcomes_keep_failed_sources_unchanged(self):
        for op,row in zip(limits(),native('inset_output_limits')['results']):
            value=row['value'];success=op['scene']=='ellipse' and op['distance']==3.
            self.assertEqual(value['results'],['Success' if success else 'Failure'])
            if not success:
                self.assertEqual(len(value['after']),1)
                self.assertEqual(value['before'][0]['brep'],value['after'][0]['brep'])

    def test_unowned_and_nonboolean_recipes_decline(self):
        for op in [cases()[0],limits()[0],PREFERENCES]:
            validate(op)
            for changes in ({'id':'unowned'},{'loose':1},{'split':1},{'join_outputs':0},{'extra':True}):
                with self.assertRaises(ValueError):validate(dict(op,**changes))


if __name__=='__main__':unittest.main()
