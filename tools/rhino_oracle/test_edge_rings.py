"""Frozen ring controls, invalid recipe rejection and standalone worker imports."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from .edge_ring_cases import cases,discovery_cases,ngon_cases
from .structural_sdk import stage_helpers,validate_operations
HERE=Path(__file__).resolve().parent

class EdgeRingTests(unittest.TestCase):
    def test_complete_native_states_sources_and_redo(self):
        count=0
        for family,recipes in [('edge_ring_discovery',discovery_cases()),('edge_ring_commands',cases()),('edge_ring_ngons',ngon_cases())]:
            fixture=HERE/'fixtures'/(family+'.json');request=json.loads(fixture.read_bytes());validate_operations(request)
            self.assertEqual(request['operations'],recipes)
            data=json.loads(gzip.decompress((HERE/'observations'/(family+'.json.gz')).read_bytes()))
            self.assertEqual([r['id'] for r in data['results']],[o['id'] for o in recipes])
            self.assertEqual(data['provenance']['request_sha256'],hashlib.sha256(fixture.read_bytes()).hexdigest())
            for name,digest in data['provenance']['sources'].items():
                raw=gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(raw).hexdigest(),digest,name)
            for op,row in zip(recipes,data['results']):
                count+=1;v=row['value'];self.assertIs(v['source_unchanged'],True)
                self.assertEqual(len(v['trace']),len(op['steps']))
                self.assertEqual(v['success'],op['finish']=='Enter')
                for stage,objects in [('after',1),('redo',2),('undo',1)]:
                    self.assertEqual(v[stage]['objects'],objects)
                    for key in ['whole','components','ordinals']:self.assertEqual(v[stage][key],v['after'][key])
        self.assertEqual(count,85)

    def test_owned_recipes_and_isolated_worker_import(self):
        for recipe in discovery_cases()+cases()+ngon_cases():
            for change in [dict(pair=[6.,7.]),dict(whole=1),dict(extra=True),dict(scene='external')]:
                with self.assertRaises(ValueError):validate_operations(dict(operations=[dict(copy.deepcopy(recipe),**change)]))
        request=dict(operations=cases()+ngon_cases())
        with tempfile.TemporaryDirectory() as folder:
            stage_helpers(request,HERE,Path(folder))
            subprocess.run([sys.executable,'-I','-c','import sys;sys.path.insert(0,sys.argv[1]);import edge_ring_probe',folder],check=True,capture_output=True)

if __name__=='__main__':unittest.main()
