"""Immutable native SubD joins, including both command observation phases."""
import gzip
import hashlib
import json
import unittest
from .capture_family import HERE
from .generate_subd_join import request
from .subd_join_cases import validate
from .subd_join_command_probe import validate as command_validate


class SubDJoinTests(unittest.TestCase):
    def test_frozen_coverage_and_provenance(self):
        for family,args in [('subd_join_sdk',(False,False)),('subd_join_command',(True,False)),('subd_join_history',(True,True))]:
            fixture=HERE/('fixtures/'+family+'.json')
            query=json.loads(fixture.read_text());native=json.loads(gzip.decompress((HERE/('observations/'+family+'.json.gz')).read_bytes()))
            self.assertEqual(query,request(*args))
            self.assertEqual(native['provenance']['request_sha256'],hashlib.sha256(fixture.read_bytes()).hexdigest())
            self.assertEqual([r['id'] for r in native['results']],[op['id'] for op in query['operations']])
            for sha in native['provenance']['sources'].values():
                original=gzip.decompress((HERE/('observations/source_snapshots/'+sha+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(original).hexdigest(),sha)
            for row in native['results']:
                self.assertNotIn('error',row)
                if family=='subd_join_sdk':self.assertTrue(row['value']['source_unchanged'])
                elif family=='subd_join_history':self.assertEqual(row['value']['command_depth'],0)

    def test_script_return_selection_is_distinct_and_does_not_change_geometry(self):
        native=json.loads(gzip.decompress((HERE/'observations/subd_join_history.json.gz').read_bytes()))
        differences=[]
        for row in native['results']:
            after=row['value']['after'];returned=row['value']['after_script']
            self.assertEqual(len(after),len(returned))
            for first,last in zip(after,returned):
                self.assertEqual({k:v for k,v in first.items() if k!='selected'},
                                 {k:v for k,v in last.items() if k!='selected'})
            if after!=returned:differences.append(row['id'])
        self.assertTrue(differences)
        self.assertTrue(all('-post-' in name for name in differences))

    def test_only_fixed_owned_recipes_are_admitted(self):
        for args,check in [((False,False),validate),((True,False),command_validate),((True,True),command_validate)]:
            op=request(*args)['operations'][0]
            for changes in ({'scene':'../escape'},{'id':'other'},{'crease':1},{'extra':True}):
                with self.assertRaises(ValueError):check(dict(op,**changes))


if __name__=='__main__':unittest.main()
