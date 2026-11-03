"""Immutable offset captures, producer provenance and owned recipe validation."""
import gzip
import hashlib
import json
import unittest
from .capture_family import HERE
from .generate_subd_offset import request
from .generate_subd_offset_command import query as command_query
from .generate_subd_offset_preferences import query as preference_query
from .generate_subd_offset_batch import query as batch_query
from .subd_offset_cases import validate as sdk_validate
from .subd_offset_command_cases import validate as command_validate
from .subd_offset_batch import validate as batch_validate


def native(family):
    return json.loads(gzip.decompress((HERE/('observations/'+family+'.json.gz')).read_bytes()))


class SubDOffsetTests(unittest.TestCase):
    def test_frozen_coverage_and_producers(self):
        for family,query in [('subd_offset_sdk',request()),('subd_offset_command',command_query),
                             ('subd_offset_preferences',preference_query),('subd_offset_batch',batch_query)]:
            fixture=HERE/('fixtures/'+family+'.json');record=native(family)
            self.assertEqual(json.loads(fixture.read_bytes()),query)
            self.assertEqual(record['provenance']['request_sha256'],hashlib.sha256(fixture.read_bytes()).hexdigest())
            self.assertEqual([r['id']for r in record['results']],[op['id']for op in query['operations']])
            for sha in record['provenance']['sources'].values():
                data=gzip.decompress((HERE/('observations/source_snapshots/'+sha+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(data).hexdigest(),sha)
            for row in record['results']:
                self.assertNotIn('error',row)
                if family=='subd_offset_sdk':self.assertTrue(row['value']['source_unchanged'])
                elif family!='subd_offset_preferences':self.assertEqual(row['value']['results'],['Success'])

    def test_cleanup_changes_selection_only_and_outputs_start_unselected(self):
        for family in ('subd_offset_command','subd_offset_batch'):
            for row in native(family)['results']:
                value=row['value']
                self.assertEqual([{k:v for k,v in r.items()if k!='selected'}for r in value['after']],
                                 [{k:v for k,v in r.items()if k!='selected'}for r in value['after_script']])
                self.assertTrue(all(not r['selected']for r in value['after']if r['source']is None))
                self.assertTrue(all(not r['selected']for r in value['after_script']))

    def test_fixed_recipes_reject_unowned_inputs_and_implicit_boolean_coercions(self):
        for check,query in [(sdk_validate,request()),(command_validate,command_query),(batch_validate,batch_query)]:
            op=query['operations'][0]
            for change in ({'id':'other'},{'scene':[]},{'solid':1},{'extra':True}):
                with self.assertRaises(ValueError):check(dict(op,**change))


if __name__=='__main__':unittest.main()
