"""Native straight-sweep storage, independent split stages and immutable producers."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from .extrusion_splitting_cases import pilot, holdouts, confirmations, cancellations, curved, mixed
from .structural_sdk import stage_helpers, validate_operations
from .subd_edge_analysis_replay import HERE, read

FAMILIES = dict(extrusion_splitting_pilot=pilot, extrusion_splitting_holdouts=holdouts, extrusion_splitting_confirmations=confirmations, extrusion_splitting_cancellations=cancellations, extrusion_splitting_curved=curved, extrusion_splitting_mixed=mixed)


class ExtrusionSplittingTests(unittest.TestCase):
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
                if op['op']!='extrusion_splitting':continue
                count+=1;v=row['value'];self.assertTrue(v['success']);self.assertNotIn('Unknown command:',v['history'])
                self.assertEqual([e['result'] for e in v['events']],['Success'])
                self.assertEqual(v['created'],v['after']);self.assertEqual(v['before'],v['undo']);self.assertEqual(v['after'],v['redo'])
                self.assertEqual(len(v['after']),1 if op['delete_input'] else 2)
                if not op['delete_input']:self.assertEqual(v['before'][0],v['after'][0])
                out=v['after'][-1];self.assertFalse(out['source']);self.assertEqual(out['selected'],0);self.assertIn(out['kind'],['brep','extrusion'])
        self.assertEqual(count,163)

    def test_split_preferences_and_cancelled_edits(self):
        rows={r['id']:r['value']['after'][-1]for r in read('extrusion_splitting_pilot')['results']}
        faces=lambda name:len(rows[name]['brep']['faces'])
        self.assertEqual([faces('tangent_'+p)for p in ['00','01','10','11']],[1,2,1,2])
        self.assertEqual([faces('corner_'+p)for p in ['00','01','10','11']],[1,2,2,2])
        self.assertEqual(faces('tangent_default'),2)
        rows={r['id']:r['value']['after'][-1]for r in read('extrusion_splitting_confirmations')['results']}
        self.assertEqual([len(rows['tangent_hidden_1'+p]['brep']['faces'])for p in ['0','1']],[1,2])
        self.assertEqual(len(rows['tangent_hidden_mode']['brep']['faces']),1)
        for row in read('extrusion_splitting_cancellations')['results']:
            v=row['value'];self.assertFalse(v['seed']['success']);self.assertEqual(v['seed']['objects'],1);self.assertEqual(v['seed']['in_command'],0);self.assertEqual(len(v['after'][-1]['brep']['faces']),2)
        rows={r['id']:r['value']['after'][-1]for r in read('extrusion_splitting_curved')['results']}
        faces=rows['closed_rational_positive_01']['brep']['faces']
        self.assertEqual(faces[2]['definition']['degree'],[1,1])
        self.assertEqual([f['definition']['domain_u']for f in faces if f['definition']['degree']==[2,1]],[[-4.,-3.],[-3.,-2.],[-2.,-1.],[-1.,0.]])

    def test_malformed_owned_recipes_are_rejected(self):
        for edit in [dict(id='unowned'),dict(solid=1),dict(delete_input=1),dict(extra=True),dict(curve=None)]:
            with self.assertRaises(ValueError):validate_operations(dict(operations=[dict(pilot()[0],**edit)]))
        op=copy.deepcopy(pilot()[0]);op['curve']['control_points'][0]['weight']=True
        with self.assertRaises(ValueError):validate_operations(dict(operations=[op]))
    def test_helpers_import_in_isolation(self):
        for op in [pilot()[0],holdouts()[-1]]:
            with tempfile.TemporaryDirectory() as directory:
                stage_helpers(dict(operations=[op]),HERE,Path(directory))
                module='extrusion_splitting_probe'
                subprocess.run([sys.executable,'-I','-c','import sys;sys.path.insert(0,sys.argv[1]);__import__(sys.argv[2])',directory,module],check=True,capture_output=True)


if __name__=='__main__':unittest.main()
