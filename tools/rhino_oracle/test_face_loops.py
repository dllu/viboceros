"""Complete face/edge state, immutable producers and mixed-selection controls."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from .face_loop_cases import discovery_cases,cases,ngon_cases
from .component_pattern_cases import cases as mixed,nonmanifold_cases
from .structural_sdk import stage_helpers,validate_operations
HERE=Path(__file__).resolve().parent
FAMILIES=[('face_loop_discovery',discovery_cases()),('face_loop_commands',cases()),
          ('component_pattern_preselection',mixed()),('component_pattern_holdouts',ngon_cases()+nonmanifold_cases())]

class FaceLoopTests(unittest.TestCase):
    def test_complete_states_provenance_and_selection_outside_model_history(self):
        count=0
        for family,recipes in FAMILIES:
            fixture=HERE/'fixtures'/(family+'.json');request=json.loads(fixture.read_bytes());validate_operations(request)
            self.assertEqual(request['operations'],recipes)
            data=json.loads(gzip.decompress((HERE/'observations'/(family+'.json.gz')).read_bytes()))
            self.assertEqual([r['id']for r in data['results']],[o['id']for o in recipes])
            self.assertEqual(data['provenance']['request_sha256'],hashlib.sha256(fixture.read_bytes()).hexdigest())
            for name,digest in data['provenance']['sources'].items():
                raw=gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(raw).hexdigest(),digest,name)
            for op,row in zip(recipes,data['results']):
                count+=1;v=row['value'];self.assertIs(v['source_unchanged'],True)
                self.assertEqual(len(v['trace']),len(op['steps']));self.assertEqual(v['success'],op['finish']=='Enter')
                for stage,objects in [('after',1),('redo',2),('undo',1)]:
                    self.assertEqual(v[stage]['objects'],objects)
                    for key in ['whole','components','ordinals','face_ordinals']:self.assertEqual(v[stage][key],v['after'][key])
        self.assertEqual(count,131)

    def test_restricted_recipes_and_standalone_worker_imports(self):
        for family,recipes in FAMILIES:
            for recipe in recipes:
                for change in [dict(pair=[6.,7.]),dict(whole=1),dict(extra=True),dict(scene='external')]:
                    with self.assertRaises(ValueError):validate_operations(dict(operations=[dict(copy.deepcopy(recipe),**change)]))
            with tempfile.TemporaryDirectory() as folder:
                request=dict(operations=recipes);stage_helpers(request,HERE,Path(folder))
                subprocess.run([sys.executable,'-I','-c','import sys;sys.path.insert(0,sys.argv[1]);import face_loop_probe,component_pattern_probe' if family=='component_pattern_holdouts' else 'import sys;sys.path.insert(0,sys.argv[1]);import '+('component_pattern_probe' if family=='component_pattern_preselection' else 'face_loop_probe'),folder],check=True,capture_output=True)

if __name__=='__main__':unittest.main()
