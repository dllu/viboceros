"""Nested Length getters retain native lifecycle evidence and frozen producers."""
import copy
import gzip
import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from .length_chain_cases import cases, controls, followups
from .structural_sdk import stage_helpers, validate_operations

HERE=Path(__file__).resolve().parent
FAMILIES=[('discovery',cases()),('controls',controls()),('followups',followups())]

class LengthChainTests(unittest.TestCase):
    def test_fixed_native_cases_selection_redo_and_frozen_sources(self):
        count=0
        for family,recipes in FAMILIES:
            name='length_chain_'+family
            fixture=HERE/'fixtures'/(name+'.json')
            request=json.loads(fixture.read_bytes())
            validate_operations(request)
            self.assertEqual(request['operations'],recipes)
            path=HERE/'observations'/(name+'.json')
            data=json.loads(path.read_bytes() if path.exists() else gzip.decompress(path.with_suffix('.json.gz').read_bytes()))
            self.assertEqual(data['provenance']['request_sha256'],hashlib.sha256(fixture.read_bytes()).hexdigest())
            for file,digest in data['provenance']['sources'].items():
                raw=gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(raw).hexdigest(),digest,file)
            self.assertEqual(len(data['results']),len(recipes))
            for op,row in zip(recipes,data['results'],strict=True):
                count+=1;value=row['value']
                self.assertEqual(row['id'],op['id'])
                self.assertTrue(value['source_unchanged'])
                self.assertEqual(len(value['trace']),len(op['steps']))
                for stage in ('after','redo','undo'):
                    self.assertEqual(value[stage]['whole'],[])
                    self.assertEqual(value[stage]['components'],[])
                    self.assertEqual(value[stage]['objects'],len(value['input'])+(stage=='redo'))
                if op['id']=='units_after_chain':
                    self.assertIn('Unknown command: Units=Centimeters',value['history'])
                else:
                    self.assertNotIn('Unknown command',value['history'])
        self.assertEqual(count,17)

    def test_owned_validation_and_isolated_worker_imports(self):
        for _,recipes in FAMILIES:
            for recipe in recipes:
                for change in [dict(seed=True),dict(seed=1.),dict(pre=[0]),dict(scene='external'),dict(extra=True)]:
                    with self.assertRaises(ValueError):
                        validate_operations(dict(operations=[dict(copy.deepcopy(recipe),**change)]))
        request=dict(operations=[r for _,rows in FAMILIES for r in rows])
        with tempfile.TemporaryDirectory() as folder:
            stage_helpers(request,HERE,Path(folder))
            subprocess.run([sys.executable,'-I','-c',
                'import sys;sys.path.insert(0,sys.argv[1]);import length_chain_probe',folder],
                check=True,capture_output=True)

if __name__=='__main__':unittest.main()
