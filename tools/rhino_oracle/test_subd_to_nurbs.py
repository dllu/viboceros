"""Frozen SubD conversion evidence, topology and isolated producer imports."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from . import subd_to_nurbs_probe as probe
from .structural_sdk import stage_helpers, validate_operations

HERE = Path(__file__).resolve().parent


class SubDToNurbsTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.query = json.loads((HERE/'fixtures/subd_to_nurbs_sdk.json').read_bytes())
        cls.native = json.loads(gzip.decompress((HERE/'observations/subd_to_nurbs_sdk.json.gz').read_bytes()))

    def test_immutable_request_results_and_producer_snapshots(self):
        self.assertEqual(self.query['operations'], probe.cases())
        validate_operations(self.query)
        self.assertEqual([r['id'] for r in self.native['results']], [o['id'] for o in probe.cases()])
        self.assertEqual(len(self.native['results']), 30)
        provenance = self.native['provenance']
        self.assertEqual(provenance['request_sha256'], hashlib.sha256((HERE/'fixtures/subd_to_nurbs_sdk.json').read_bytes()).hexdigest())
        for name, digest in provenance['sources'].items():
            frozen = gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(frozen).hexdigest(), digest, name)
        for row in self.native['results']:
            value = row['value']
            self.assertIs(value['source_unchanged'], True)
            self.assertIs(value['before']['valid'], True)
            self.assertIs(value['output']['valid'], True)
            archive = bytes(value['archive'])
            self.assertTrue(archive.startswith(b'3D Geometry File Format '))

    def test_packing_ngon_sectors_and_extraordinary_vertex_bases(self):
        by_id = {r['id']: r['value'] for r in self.native['results']}
        for scene, unpacked, packed in [('quad_corners',1,1),('quad_smooth',1,1),
                ('grid',12,1),('box',6,6),('pentagon',5,5),('creased_box',6,6),('torus',32,1)]:
            for pack, count in [(False,unpacked),(True,packed)]:
                value=by_id[scene+'_'+str(pack)+'_3']
                self.assertEqual(len(value['output']['geometry']['faces']), count)
                self.assertEqual(value['output']['solid'], scene in ('box','creased_box','torus'))
        for mode in range(5):
            expected=[13,13] if mode==4 else [11,11]
            for packed in (False,True):
                for face in by_id['box_'+str(packed)+'_'+str(mode)]['output']['geometry']['faces']:
                    self.assertEqual(face['definition']['degree'],[3,3])
                    self.assertEqual(face['definition']['control_count'],expected)
        self.assertEqual(len(by_id['pentagon_False_3']['before']['faces']),1)
        for value in by_id.values():
            for edge in value['edges']:
                self.assertEqual(len(edge['curves']),2)
                for curve in edge['curves']:
                    self.assertIsNotNone(curve)
                    self.assertEqual(curve['degree'],3)
                    self.assertTrue(all(p['weight']==1 for p in curve['control_points']))

    def test_exact_allowlist_and_standalone_worker_import(self):
        for key, invalid in [('scene','external'),('vertex',True),('vertex',3.0),('vertex',5),('packed',1),('id','external')]:
            op=copy.deepcopy(probe.cases()[0]);op[key]=invalid
            with self.subTest(key=key,value=invalid), self.assertRaises(ValueError):
                validate_operations(dict(self.query,operations=[op]))
        with tempfile.TemporaryDirectory() as folder:
            stage_helpers(self.query,HERE,Path(folder))
            subprocess.run([sys.executable,'-I','-c',
                'import sys;sys.path.insert(0,sys.argv[1]);import subd_to_nurbs_probe',folder],
                check=True,capture_output=True)


if __name__=='__main__':
    unittest.main()
