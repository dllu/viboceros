"""Frozen SubD extraction coverage, independent inputs and archived producers."""
import gzip
import hashlib
import json
import unittest
from .capture_family import HERE
from .generate_subd_extract import request
from .generate_subd_extract_sources import query as sources
from .generate_subd_extract_preferences import query as preferences
from .subd_extract_cases import validate
from .subd_extract_sources import validate as source_validate


def observation(family):
    return json.loads(gzip.decompress((HERE/('observations/'+family+'.json.gz')).read_bytes()))


class SubDExtractTests(unittest.TestCase):
    def test_frozen_coverage_provenance_and_native_success(self):
        for family,query in [('subd_extract_command',request()),('subd_extract_details',request(True)),
                             ('subd_extract_sources',sources),('subd_extract_preferences',preferences)]:
            fixture=HERE/('fixtures/'+family+'.json');native=observation(family)
            self.assertEqual(json.loads(fixture.read_bytes()),query)
            self.assertEqual(native['provenance']['request_sha256'],hashlib.sha256(fixture.read_bytes()).hexdigest())
            self.assertEqual([r['id'] for r in native['results']],[op['id'] for op in query['operations']])
            for sha in native['provenance']['sources'].values():
                data=gzip.decompress((HERE/('observations/source_snapshots/'+sha+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(data).hexdigest(),sha)
            for row in native['results']:
                self.assertNotIn('error',row)
                if family in ('subd_extract_command','subd_extract_details'):
                    self.assertTrue(row['value']['success']);self.assertEqual(row['value']['results'],['Success'])

    def test_script_cleanup_releases_only_component_picks(self):
        for family in ('subd_extract_command','subd_extract_details'):
            for row in observation(family)['results']:
                value=row['value']
                def geometry(rows):return [{k:v for k,v in r.items() if k!='selected_faces'}for r in rows]
                self.assertEqual(geometry(value['after']),geometry(value['after_script']))
                self.assertTrue(all(not r['selected_faces'] for r in value['after_script']))

    def test_only_fixed_owned_inputs_are_admitted(self):
        for check,query in [(validate,request()),(source_validate,sources)]:
            op=query['operations'][0]
            for change in ({'scene':'../escape'},{'scene':[]},{'id':'other'},{'extra':True}):
                with self.assertRaises(ValueError):check(dict(op,**change))


if __name__=='__main__':unittest.main()
