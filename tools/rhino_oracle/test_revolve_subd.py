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
from .revolve_subd_cases import pilot, holdouts, edge_cases, policy_cases
from .revolve_subd_profile_probe import request
from .structural_sdk import stage_helpers, validate_operations
from .subd_edge_analysis_replay import HERE, read

FAMILIES = dict(revolve_subd_pilot=pilot, revolve_subd_edges=edge_cases,
                revolve_subd_policy=policy_cases,
                revolve_subd_holdouts=lambda: holdouts()+request(True)['operations'],
                revolve_subd_profiles=lambda: request()['operations'])


class RevolveSubDTests(unittest.TestCase):
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
                if op['op']!='revolve_subd':continue
                count+=1;v=row['value'];self.assertTrue(v['success']);self.assertNotIn('Unknown command:',v['history'])
                self.assertEqual([e['result'] for e in v['events']],['Success'])
                self.assertEqual(v['created'],v['after']);self.assertEqual(v['before'],v['undo']);self.assertEqual(v['after'],v['redo'])
                self.assertEqual(len(v['after']),1 if op['delete_input'] else 2)
                if not op['delete_input']:self.assertEqual(v['before'][0],v['after'][0])
                out=v['after'][-1];self.assertFalse(out['source']);self.assertEqual(out['selected'],0);self.assertEqual(out['kind'],'subd');self.assertTrue(out['subd']['valid'])
                self.assertTrue(out['subd']['surface_mesh']['faces'])
        self.assertEqual(count,80)

    def test_count_floor_and_closed_and_pole_topology(self):
        for row in read('revolve_subd_edges')['results']:
            if row['id'].startswith('line_count_'):
                self.assertEqual(len(row['value']['after'][-1]['subd']['faces']),4)
        results={r['id']:r['value']['after'][-1]['subd'] for r in read('revolve_subd_pilot')['results']}
        self.assertEqual(len(results['line_full']['faces']),8)
        self.assertEqual(len(results['line_partial']['faces']),8)
        self.assertEqual(results['line_partial']['vertex_tags'].count(3),4)
        self.assertEqual(len(results['poles_full']['faces']),40)
        self.assertEqual(results['poles_full']['edge_tags'].count(2),0)
        # Native fitting deliberately replaces a single cubic Bezier by a line.
        self.assertEqual(results['cubic_full'],results['line_full'])

    def test_malformed_owned_recipes_are_rejected(self):
        for edit in [dict(id='unowned'),dict(segment_count=True),dict(delete_input=1),dict(extra=True),dict(curve=None)]:
            with self.assertRaises(ValueError):validate_operations(dict(operations=[dict(pilot()[0],**edit)]))
        op=copy.deepcopy(pilot()[0]);op['curve']['control_points'][0]['weight']=True
        with self.assertRaises(ValueError):validate_operations(dict(operations=[op]))
        op=copy.deepcopy(policy_cases()[0]);op['seed_surface']=1
        with self.assertRaises(ValueError):validate_operations(dict(operations=[op]))

    def test_helpers_import_in_isolation(self):
        for op in [pilot()[0],request()['operations'][0]]:
            with tempfile.TemporaryDirectory() as directory:
                stage_helpers(dict(operations=[op]),HERE,Path(directory))
                module='revolve_subd_probe' if op['op']=='revolve_subd' else 'revolve_subd_profile_probe'
                subprocess.run([sys.executable,'-I','-c','import sys;sys.path.insert(0,sys.argv[1]);__import__(sys.argv[2])',directory,module],check=True,capture_output=True)


if __name__=='__main__':unittest.main()
