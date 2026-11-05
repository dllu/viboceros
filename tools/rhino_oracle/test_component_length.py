"""Owned mesh/SubD measurement cases, native state and immutable provenance."""
import copy,gzip,hashlib,json,subprocess,sys,tempfile,unittest
from pathlib import Path
from .component_length_cases import cases,followups,workflows,source
from .structural_sdk import validate_operations,stage_helpers
HERE=Path(__file__).resolve().parent
FAMILIES=[('discovery',cases()),('followups',followups()),('workflows',workflows())]

class ComponentLengthTests(unittest.TestCase):
    def test_native_geometry_selection_history_and_frozen_sources(self):
        count=0
        for family,recipes in FAMILIES:
            name='component_length_'+family;fixture=HERE/'fixtures'/(name+'.json')
            request=json.loads(fixture.read_bytes());validate_operations(request)
            self.assertEqual(request['operations'],recipes)
            path=HERE/'observations'/(name+'.json')
            data=json.loads(path.read_bytes()if path.exists()else gzip.decompress(path.with_suffix('.json.gz').read_bytes()))
            self.assertEqual(data['provenance']['request_sha256'],hashlib.sha256(fixture.read_bytes()).hexdigest())
            for file,digest in data['provenance']['sources'].items():
                raw=gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(raw).hexdigest(),digest,file)
            self.assertEqual(len(data['results']),len(recipes))
            for op,row in zip(recipes,data['results'],strict=True):
                count+=1;v=row['value'];self.assertEqual(row['id'],op['id'])
                expected=source(op['scene']);self.assertEqual(v['input'].keys(),expected.keys())
                for key in expected:
                    if key!='points':self.assertEqual(v['input'][key],expected[key])
                self.assertEqual(len(v['input']['points']),len(expected['points']))
                for actual,point in zip(v['input']['points'],expected['points'],strict=True):
                    for a,b in zip(actual,point,strict=True):self.assertAlmostEqual(a,b,delta=1e-12)
                self.assertTrue(v['source_unchanged']);self.assertNotIn('Unknown command',v['history'])
                self.assertEqual(v['after']['components'],[])
                self.assertEqual(v['after']['curve'],op['curve'] and not op.get('steps'))
                for key,offset in [('after',0),('redo',1),('undo',0)]:
                    self.assertEqual(v[key]['objects'],1+op['curve']+offset)
                    self.assertEqual(v[key]['components'],[])
                    self.assertEqual(v[key]['curve'],v['after']['curve'])
                if op.get('steps'):self.assertEqual(len(v['trace']),len(op['steps']))
        self.assertEqual(count,42)

    def test_owned_case_bounds_and_isolated_worker_imports(self):
        for _,recipes in FAMILIES:
            for recipe in recipes:
                for change in [dict(flat=1),dict(curve=0),dict(pairs=[[True,1]]),dict(kind='external'),dict(scene='external'),dict(extra=True)]:
                    with self.assertRaises(ValueError):validate_operations(dict(operations=[dict(copy.deepcopy(recipe),**change)]))
        request=dict(operations=[op for _,recipes in FAMILIES for op in recipes])
        with tempfile.TemporaryDirectory()as folder:
            stage_helpers(request,HERE,Path(folder))
            subprocess.run([sys.executable,'-I','-c','import sys;sys.path.insert(0,sys.argv[1]);import component_length_probe',folder],check=True,capture_output=True)

if __name__=='__main__':unittest.main()
