"""Frozen native face-pack toggles, owned inputs and settled command history."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from .subd_nurbs_packs_probe import cases,discovery_cases
from .structural_sdk import stage_helpers,validate_operations
HERE=Path(__file__).resolve().parent
class SubDNurbsPacksTests(unittest.TestCase):
    def test_frozen_commands_sources_and_geometry_coverage(self):
        count=outputs=0
        for family,recipes in [('subd_nurbs_packs_discovery',discovery_cases()),('subd_nurbs_packs',cases())]:
            path=HERE/'fixtures'/(family+'.json');query=json.loads(path.read_bytes())
            data=json.loads(gzip.decompress((HERE/'observations'/(family+'.json.gz')).read_bytes()))
            self.assertEqual(query['operations'],recipes);validate_operations(query)
            self.assertEqual([r['id'] for r in data['results']],[o['id'] for o in recipes])
            self.assertEqual(data['provenance']['request_sha256'],hashlib.sha256(path.read_bytes()).hexdigest())
            for name,digest in data['provenance']['sources'].items():
                raw=gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(raw).hexdigest(),digest,name)
            for row in data['results']:
                count+=1;v=row['value'];cancel=row['id'].endswith('cancel')
                self.assertEqual(v['success'],not cancel);self.assertEqual(v['results'],['Cancel' if cancel else 'Success'])
                self.assertEqual([o['source'] for o in v['before']],[o['source'] for o in v['undo']])
                for o in v['settled']:
                    if o['kind']=='Brep':
                        outputs+=1
                        expected=24 if row['id']=='force_unpacked' else 6 if row['id']=='force_accept' or row['id'].startswith('mixed_') else 2
                        self.assertEqual(len(o['brep']['faces']),expected)
                    if o['kind']=='SubD':self.assertEqual(o['crc'],v['before'][0]['crc'])
                for snapshot in ('settled','redo'):
                    self.assertEqual([(o['source'],o['kind'],o['selected']) for o in v[snapshot]],[(o['source'],o['kind'],o['selected']) for o in v['settled']])
                if cancel:self.assertEqual(len(v['settled']),len(v['before']))
        self.assertEqual((count,outputs),(14,10))
    def test_fixed_recipes_and_standalone_worker_import(self):
        query=json.loads((HERE/'fixtures/subd_nurbs_packs.json').read_bytes())
        for change in [dict(case='external'),dict(extra=True),dict(delete=1),dict(postselect=1)]:
            op=copy.deepcopy(cases()[0]);op.update(change)
            with self.assertRaises(ValueError):validate_operations(dict(query,operations=[op]))
        with tempfile.TemporaryDirectory() as folder:
            stage_helpers(query,HERE,Path(folder))
            subprocess.run([sys.executable,'-I','-c','import sys;sys.path.insert(0,sys.argv[1]);import subd_nurbs_packs_probe',folder],check=True,capture_output=True)
if __name__=='__main__':unittest.main()
