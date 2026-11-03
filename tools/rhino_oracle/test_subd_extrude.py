"""Owned extrusion recipes, immutable provenance, and explicit macro cleanup."""
import gzip
import hashlib
import json
import unittest
from .capture_family import HERE
from .generate_subd_extrude import request
from .subd_extrude_cases import validate

class SubDExtrudeTests(unittest.TestCase):
    def test_frozen_coverage_and_producer_provenance(self):
        for extra,mixed,total in ((False,False,82),(True,False,30),(False,True,16)):
            family='subd_extrude_mixed' if mixed else 'subd_extrude_extra' if extra else 'subd_extrude_command'
            fixture=HERE/('fixtures/'+family+'.json')
            record=json.loads(gzip.decompress((HERE/('observations/'+family+'.json.gz')).read_bytes()))
            query=request(extra,mixed)
            self.assertEqual(json.loads(fixture.read_bytes()),query)
            self.assertEqual(record['provenance']['request_sha256'],hashlib.sha256(fixture.read_bytes()).hexdigest())
            self.assertEqual(len(record['results']),total)
            self.assertEqual([r['id']for r in record['results']],[r['id']for r in query['operations']])
            for sha in record['provenance']['sources'].values():
                data=gzip.decompress((HERE/('observations/source_snapshots/'+sha+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(data).hexdigest(),sha)
            for row in record['results']:
                self.assertNotIn('error',row)
                v=row['value'];self.assertEqual(v['results'],['Success'])
                self.assertEqual([{k:x for k,x in r.items()if k!='selected'}for r in v['after']],
                                 [{k:x for k,x in r.items()if k!='selected'}for r in v['after_script']])
                self.assertTrue(all(not r['selected']for r in v['after_script']))
                self.assertTrue(all(not r['selected_faces'] and not r['selected_edges']for r in v['after']))
                self.assertTrue(all(r['subd']['valid']for r in v['after']))

    def test_rejects_unowned_and_malformed_jobs(self):
        op=request()['operations'][0]
        for change in ({'id':'other'},{'target':[]},{'distance':True},{'distance':0.},{'extra':True},{'mode':'Free'}):
            with self.assertRaises(ValueError):validate(dict(op,**change))

if __name__=='__main__':unittest.main()
