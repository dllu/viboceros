"""Frozen G1x valence and near-threshold controls retain full native geometry."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from . import subd_g1x_probe as probe
from .structural_sdk import stage_helpers,validate_operations

HERE=Path(__file__).resolve().parent

class SubDG1xTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.query=json.loads((HERE/'fixtures/subd_g1x.json').read_bytes())

    def test_fixed_requests_sources_and_native_validity(self):
        for family,options,count in [('subd_g1x',{},48),('subd_g1x_sectors',dict(sectors=True),15),
                                     ('subd_g1x_charts',dict(charts=True),21)]:
            path=HERE/'fixtures'/(family+'.json');query=json.loads(path.read_bytes())
            data=json.loads(gzip.decompress((HERE/'observations'/(family+'.json.gz')).read_bytes()))
            self.assertEqual(query['operations'],probe.cases(**options));validate_operations(query)
            self.assertEqual(len(data['results']),count)
            self.assertEqual([r['id'] for r in data['results']],[o['id'] for o in probe.cases(**options)])
            self.assertEqual(data['provenance']['request_sha256'],hashlib.sha256(path.read_bytes()).hexdigest())
            for name,digest in data['provenance']['sources'].items():
                raw=gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(raw).hexdigest(),digest,name)
            for row in data['results']:
                value=row['value'];self.assertIs(value['source_unchanged'],True)
                self.assertIs(value['before']['valid'],True);self.assertIs(value['output']['valid'],True)
            self.assertEqual({op['mode'] for op in query['operations']},{0,1,3})

    def test_strict_recipe_validation_and_standalone_import(self):
        for change in [dict(mode=True),dict(mode=3.0),dict(mode=2),dict(scene='external'),dict(extra=True)]:
            op=copy.deepcopy(probe.cases()[0]);op.update(change)
            with self.assertRaises(ValueError):validate_operations(dict(self.query,operations=[op]))
        with tempfile.TemporaryDirectory() as folder:
            stage_helpers(self.query,HERE,Path(folder))
            subprocess.run([sys.executable,'-I','-c',
                'import sys;sys.path.insert(0,sys.argv[1]);import subd_g1x_probe',folder],check=True,capture_output=True)

if __name__=='__main__':unittest.main()
