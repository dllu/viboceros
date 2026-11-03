"""Frozen continuity controls preserve distinct native processing modes."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from . import subd_continuity_probe as probe
from .structural_sdk import stage_helpers,validate_operations

HERE=Path(__file__).resolve().parent

class SubDContinuityTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.query=json.loads((HERE/'fixtures/subd_continuity.json').read_bytes())
        cls.data=json.loads(gzip.decompress((HERE/'observations/subd_continuity.json.gz').read_bytes()))

    def test_requests_sources_and_immutable_native_outputs(self):
        self.assertEqual(self.query['operations'],probe.cases());validate_operations(self.query)
        self.assertEqual(len(self.data['results']),85)
        self.assertEqual([r['id'] for r in self.data['results']],[o['id'] for o in probe.cases()])
        self.assertEqual(self.data['provenance']['request_sha256'],hashlib.sha256((HERE/'fixtures/subd_continuity.json').read_bytes()).hexdigest())
        for name,digest in self.data['provenance']['sources'].items():
            raw=gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(raw).hexdigest(),digest,name)
        for row in self.data['results']:
            value=row['value'];self.assertIs(value['source_unchanged'],True)
            self.assertIs(value['before']['valid'],True);self.assertIs(value['output']['valid'],True)
            for face in value['output']['geometry']['faces']:
                self.assertEqual(face['definition']['degree'],[3,3])

    def test_g1x_and_g1xx_cannot_be_mistaken_for_g1(self):
        values={r['id']:r['value'] for r in self.data['results']}
        first=lambda key:values[key]['output']['geometry']['faces'][0]['definition']
        self.assertNotEqual(first('seven_fan_1'),first('seven_fan_3'))
        self.assertEqual(first('box_1')['control_count'],[11,11])
        self.assertEqual(first('box_4')['control_count'],[13,13])
        self.assertIn(4,values['one_crease_box_0']['before']['vertex_tags'])

    def test_strict_recipes_and_standalone_import(self):
        for change in [dict(mode=True),dict(mode=1.0),dict(mode=5),dict(scene='external'),dict(extra=True)]:
            op=copy.deepcopy(probe.cases()[0]);op.update(change)
            with self.assertRaises(ValueError):validate_operations(dict(self.query,operations=[op]))
        with tempfile.TemporaryDirectory() as folder:
            stage_helpers(self.query,HERE,Path(folder))
            subprocess.run([sys.executable,'-I','-c',
                'import sys;sys.path.insert(0,sys.argv[1]);import subd_continuity_probe',folder],check=True,capture_output=True)

if __name__=='__main__':unittest.main()
