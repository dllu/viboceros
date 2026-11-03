"""Immutable G0 controls and holdouts; no command or performance claims."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from . import subd_g0_probe as probe
from .structural_sdk import stage_helpers,validate_operations

HERE=Path(__file__).resolve().parent


class SubDG0Tests(unittest.TestCase):
    def test_frozen_recipes_valid_outputs_and_producer_snapshots(self):
        for family,holdout in [('subd_to_nurbs_g0',False),('subd_to_nurbs_g0_holdouts',True)]:
            path=HERE/'fixtures'/(family+'.json');query=json.loads(path.read_bytes())
            self.assertEqual(query['operations'],probe.cases(holdout));validate_operations(query)
            data=json.loads(gzip.decompress((HERE/'observations'/(family+'.json.gz')).read_bytes()))
            self.assertEqual(data['provenance']['request_sha256'],hashlib.sha256(path.read_bytes()).hexdigest())
            self.assertEqual([r['id'] for r in data['results']],[o['id'] for o in query['operations']])
            for name,digest in data['provenance']['sources'].items():
                frozen=gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(frozen).hexdigest(),digest,name)
            for row in data['results']:
                value=row['value'];self.assertIs(value['source_unchanged'],True)
                self.assertIs(value['before']['valid'],True);self.assertIs(value['output']['valid'],True)
                faces=value['before']['faces'];expected=sum(1 if len(f)==4 else len(f) for f in faces)
                self.assertEqual(len(value['output']['geometry']['faces']),expected)
                for face in value['output']['geometry']['faces']:
                    self.assertEqual(face['definition']['degree'],[3,3])
                    self.assertTrue(all(c['weight']==1 for c in face['definition']['control_points']))

    def test_fixed_allowlist_and_isolated_worker_import(self):
        query=json.loads((HERE/'fixtures/subd_to_nurbs_g0_holdouts.json').read_bytes())
        for change in [dict(scene='external'),dict(id='external'),dict(extra=True),dict(scene=[])]:
            operation=copy.deepcopy(query['operations'][0]);operation.update(change)
            with self.assertRaises(ValueError):validate_operations(dict(query,operations=[operation]))
        with tempfile.TemporaryDirectory() as folder:
            stage_helpers(query,HERE,Path(folder))
            subprocess.run([sys.executable,'-I','-c',
                'import sys;sys.path.insert(0,sys.argv[1]);import subd_g0_probe',folder],
                check=True,capture_output=True)


if __name__=='__main__':unittest.main()
