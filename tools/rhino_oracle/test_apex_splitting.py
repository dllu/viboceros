"""Native apex split stages, cap charts, history and immutable producers."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from .apex_splitting_cases import pilot, holdouts, planes
from .structural_sdk import stage_helpers, validate_operations
from .subd_edge_analysis_replay import HERE, read

FAMILIES = dict(apex_splitting_pilot=pilot, apex_splitting_holdouts=holdouts, apex_splitting_planes=planes)


class ApexSplittingTests(unittest.TestCase):
    def test_requests_and_frozen_producers(self):
        for family, make in FAMILIES.items():
            path=HERE/('fixtures/'+family+'.json');query=json.loads(path.read_bytes());data=read(family)
            self.assertEqual(query['operations'],make());validate_operations(query)
            self.assertEqual([r['id'] for r in data['results']],[op['id'] for op in make()])
            self.assertEqual(data['provenance']['request_sha256'],hashlib.sha256(path.read_bytes()).hexdigest())
            for name,digest in data['provenance']['sources'].items():
                frozen=gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(frozen).hexdigest(),digest,name)

    def test_commands_keep_or_delete_sources_with_complete_history(self):
        count=0
        for family,make in FAMILIES.items():
            for op,row in zip(make(),read(family)['results'],strict=True):
                if op['op']!='apex_splitting':continue
                count+=1;v=row['value'];self.assertTrue(v['success']);self.assertNotIn('Unknown command:',v['history'])
                self.assertEqual([e['result'] for e in v['events']],['Success'])
                self.assertEqual(v['created'],v['after']);self.assertEqual(v['before'],v['undo']);self.assertEqual(v['after'],v['redo'])
                self.assertEqual(len(v['after']),1 if op['delete_input'] else 2)
                if not op['delete_input']:self.assertEqual(v['before'][0],v['after'][0])
                out=v['after'][-1];self.assertFalse(out['source']);self.assertEqual(out['selected'],0);self.assertEqual(out['kind'],'brep')
        self.assertEqual(count,71)

    def test_independent_split_stages_and_capped_face_order(self):
        rows={r['id']:r['value']['after'][-1] for r in read('apex_splitting_pilot')['results']}
        faces=lambda name:len(rows[name]['brep']['faces'])
        self.assertEqual([faces('tangent_'+p) for p in ['00','01','10','11']],[1,2,1,2])
        self.assertEqual([faces('corner_'+p) for p in ['00','01','10','11']],[1,2,2,2])
        for p in ['00','01','10','11']:
            output=rows['closed_solid_'+p];self.assertTrue(output['solid']);self.assertEqual(output['orientation'],'Outward')
            self.assertEqual(output['brep']['faces'][0]['definition']['degree'],[1,1])
        heights=lambda p:[f['definition']['domain_v'][1] for f in rows['closed_solid_'+p]['brep']['faces'][1:]]
        self.assertEqual(len(set(heights('10'))),1)
        self.assertGreater(len(set(heights('01'))),1)
        rows={r['id']:r['value']['after'][-1] for r in read('apex_splitting_holdouts')['results']}
        self.assertEqual(len(rows['tangent_default']['brep']['faces']),2)
        for i in range(6):
            self.assertEqual(len(rows['small_'+str(i)+'_10']['brep']['faces']),1 if i<2 else 2)
            self.assertEqual(len(rows['small_'+str(i)+'_01']['brep']['faces']),1 if i<5 else 2)

    def test_malformed_owned_recipes_are_rejected(self):
        for edit in [dict(id='unowned'),dict(solid=1),dict(delete_input=1),dict(extra=True),dict(curve=None)]:
            with self.assertRaises(ValueError):validate_operations(dict(operations=[dict(pilot()[0],**edit)]))
        op=copy.deepcopy(pilot()[0]);op['curve']['control_points'][0]['weight']=True
        with self.assertRaises(ValueError):validate_operations(dict(operations=[op]))
    def test_helpers_import_in_isolation(self):
        for op in [pilot()[0],holdouts()[-1]]:
            with tempfile.TemporaryDirectory() as directory:
                stage_helpers(dict(operations=[op]),HERE,Path(directory))
                module='apex_splitting_probe'
                subprocess.run([sys.executable,'-I','-c','import sys;sys.path.insert(0,sys.argv[1]);__import__(sys.argv[2])',directory,module],check=True,capture_output=True)


if __name__=='__main__':unittest.main()
