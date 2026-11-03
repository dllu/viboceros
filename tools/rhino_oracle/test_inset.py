"""Immutable inset evidence, owned input validation and getter-state provenance."""
import gzip
import hashlib
import json
import unittest
from .capture_family import HERE
from .generate_inset import request
from .generate_inset_mesh import request as mesh_request
from .inset_preferences import query
from .inset_cases import validate,validate_extra,validate_mesh,validate_mesh_confirmed,validate_mesh_primed,validate_mesh_ready

FAMILIES=[('inset_faces',request(),61),('inset_extra_faces',request(True),10),
          ('inset_mesh_faces',mesh_request(),32),('inset_mesh_confirmed',mesh_request(confirmed=True),16),
          ('inset_mesh_primed',mesh_request(primed=True),32),('inset_mesh_ready',mesh_request(ready=True),32),
          ('inset_preferences',query,1)]

def native(family):
    return json.loads(gzip.decompress((HERE/('observations/'+family+'.json.gz')).read_bytes()))

class InsetTests(unittest.TestCase):
    def test_frozen_coverage_and_content_addressed_producers(self):
        for family,request_data,count in FAMILIES:
            path=HERE/('fixtures/'+family+'.json');record=native(family)
            self.assertEqual(json.loads(path.read_bytes()),request_data)
            self.assertEqual(record['provenance']['request_sha256'],hashlib.sha256(path.read_bytes()).hexdigest())
            self.assertEqual(len(record['results']),count)
            self.assertEqual([r['id']for r in record['results']],[op['id']for op in request_data['operations']])
            for sha in record['provenance']['sources'].values():
                data=gzip.decompress((HERE/('observations/source_snapshots/'+sha+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(data).hexdigest(),sha)
            for row in record['results']:self.assertNotIn('error',row)

    def test_cleanup_and_edge_cases_are_explicit(self):
        for family,_,_ in FAMILIES[:-1]:
            for row in native(family)['results']:
                v=row['value']
                self.assertEqual([{k:x for k,x in r.items()if k!='selected_faces'}for r in v['after']],
                                 [{k:x for k,x in r.items()if k!='selected_faces'}for r in v['after_script']])
                if any(r['selected_faces']for r in v['after_script']):
                    self.assertIn('Unknown command:',v['history'])
                    self.assertEqual(family,'inset_mesh_confirmed')
                    self.assertEqual([r['selected_faces']for r in v['after_script']],[r['selected_faces']for r in v['after']])
                else:self.assertTrue(all(not r['selected_faces']for r in v['after_script']))
                if 'whole'in row['id']:
                    self.assertIn(v['results'][0],('Cancel','Failure'))
                    self.assertEqual(v['before'][0]['subd'],v['after'][0]['subd'])
                else:self.assertEqual(v['results'],['Success'])
        collapsed=next(r for r in native('inset_faces')['results']if r['id']=='quad-Group-2.0')
        self.assertFalse(collapsed['value']['after'][0]['subd']['valid'])

    def test_mesh_previous_preview_and_completed_preference_setup_are_distinct(self):
        previous=native('inset_mesh_confirmed')['results'][0]['value']
        ready=native('inset_mesh_ready')['results'][0]['value']
        self.assertEqual(previous['after'][0]['mesh']['vertices'][4][0],1.)
        self.assertAlmostEqual(ready['after'][0]['mesh']['vertices'][4][0],.3)
        for row in native('inset_mesh_ready')['results']:
            value=row['value']
            self.assertEqual(value['before'][0]['mesh']['vertices'],value['undo'][0]['mesh']['vertices'])
            self.assertEqual(value['after'][0]['mesh']['vertices'],value['redo'][0]['mesh']['vertices'])

    def test_recipes_reject_unowned_and_malformed_jobs(self):
        validators=[validate,validate_extra,validate_mesh,validate_mesh_confirmed,validate_mesh_primed,validate_mesh_ready]
        for check,(_,request_data,_)in zip(validators,FAMILIES):
            op=request_data['operations'][0]
            for changes in ({'target':[]},{'distance':True},{'extra':True},{'mode':'Other'},{'id':'unowned'}):
                with self.assertRaises(ValueError):check(dict(op,**changes))

if __name__=='__main__':unittest.main()
