"""Independent native SubD control nets, source policy and immutable producers."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from .extrude_apex_subd_cases import pilot, holdouts, diagnostics, confirmations
from .structural_sdk import stage_helpers, validate_operations
from .subd_edge_analysis_replay import HERE, read

FAMILIES = dict(extrude_apex_subd_pilot=pilot, extrude_apex_subd_holdouts=lambda: holdouts()+diagnostics(), extrude_apex_subd_confirmations=confirmations)


class ExtrudeApexSubDTests(unittest.TestCase):
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
                if op['op']!='extrude_apex_subd':continue
                count+=1;v=row['value'];self.assertTrue(v['success']);self.assertNotIn('Unknown command:',v['history'])
                self.assertEqual([e['result'] for e in v['events']],['Success'])
                self.assertEqual(v['created'],v['after']);self.assertEqual(v['before'],v['undo']);self.assertEqual(v['after'],v['redo'])
                self.assertEqual(len(v['after']),1 if op['delete_input'] else 2)
                if not op['delete_input']:self.assertEqual(v['before'][0],v['after'][0])
                out=v['after'][-1];self.assertFalse(out['source']);self.assertEqual(out['selected'],0);self.assertIn(out['kind'],['subd','brep'])
                if out['kind']=='subd':
                    self.assertTrue(out['subd']['valid']);self.assertTrue(out['subd']['surface_mesh']['faces'])
                else:self.assertFalse(out['solid'])
        self.assertEqual(count,60)

    def test_fallback_caps_and_global_crease_splitting(self):
        results={r['id']:r['value']['after'][-1]for r in read('extrude_apex_subd_pilot')['results']}
        self.assertEqual(results['closed'],results['closed_solid'])
        self.assertEqual(results['line']['kind'],'brep');self.assertEqual(len(results['closed']['surfaces']),4)
        rows={r['id']:r['value']for r in read('extrude_apex_subd_holdouts')['results']}
        self.assertIsNone(rows['line_sdk']['subd']);self.assertIsNone(rows['closed_sdk']['subd'])
        self.assertEqual(len(rows['closed_no_crease']['after'][-1]['surfaces']),1)
        self.assertEqual(len(rows['polyline_no_crease']['after'][-1]['surfaces']),1)
        self.assertEqual(len(rows['line_degree_2']['after'][-1]['subd']['faces']),4)
        self.assertEqual(len(rows['line_degree_4']['after'][-1]['subd']['faces']),8)

    def test_malformed_owned_recipes_are_rejected(self):
        for edit in [dict(id='unowned'),dict(solid=1),dict(delete_input=1),dict(extra=True),dict(curve=None)]:
            with self.assertRaises(ValueError):validate_operations(dict(operations=[dict(pilot()[0],**edit)]))
        op=copy.deepcopy(pilot()[0]);op['curve']['control_points'][0]['weight']=True
        with self.assertRaises(ValueError):validate_operations(dict(operations=[op]))
    def test_helpers_import_in_isolation(self):
        for op in [pilot()[0],holdouts()[-1]]:
            with tempfile.TemporaryDirectory() as directory:
                stage_helpers(dict(operations=[op]),HERE,Path(directory))
                module='extrude_apex_subd_probe'
                subprocess.run([sys.executable,'-I','-c','import sys;sys.path.insert(0,sys.argv[1]);__import__(sys.argv[2])',directory,module],check=True,capture_output=True)


if __name__=='__main__':unittest.main()
