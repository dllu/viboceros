"""Immutable packed/unpacked geometry, public pack IDs and strict owned recipes."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from . import subd_packing_cases as base,subd_packing_holdouts as holdouts
from .structural_sdk import stage_helpers,validate_operations

HERE=Path(__file__).resolve().parent

class SubDPackingTests(unittest.TestCase):
    def test_frozen_recipes_sources_and_valid_immutable_native_outputs(self):
        for family,operations,count in [('subd_packing',base.cases(),55),
                ('subd_packing_holdouts',holdouts.cases(),21),('subd_packing_modes',base.mode_cases(),4)]:
            path=HERE/'fixtures'/(family+'.json');query=json.loads(path.read_bytes())
            data=json.loads(gzip.decompress((HERE/'observations'/(family+'.json.gz')).read_bytes()))
            self.assertEqual(query['operations'],operations);validate_operations(query)
            self.assertEqual(len(data['results']),count)
            self.assertEqual([r['id'] for r in data['results']],[o['id'] for o in operations])
            self.assertEqual(data['provenance']['request_sha256'],hashlib.sha256(path.read_bytes()).hexdigest())
            for name,digest in data['provenance']['sources'].items():
                raw=gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(raw).hexdigest(),digest,name)
            for row in data['results']:
                value=row['value'];self.assertIs(value['source_unchanged'],True);self.assertIs(value['before']['valid'],True)
                expected=sum(1 if len(f)==4 else len(f) for f in value['before']['faces'])
                self.assertEqual(len(value['unpacked']['geometry']['faces']),expected)
                self.assertEqual(len(set(value['unpacked']['pack_ids'])),value['pack_count'])
                for output in ('unpacked','output'):
                    self.assertIs(value[output]['valid'],True)
                    self.assertEqual(len(value[output]['pack_ids']),len(value[output]['geometry']['faces']))
                    self.assertTrue(all(i>0 for i in value[output]['pack_ids']))
                self.assertLessEqual(len(value['output']['geometry']['faces']),expected)

    def test_holdouts_distinguish_crease_topology_and_coincident_components(self):
        values={r['id']:r['value'] for r in json.loads(gzip.decompress(
            (HERE/'observations/subd_packing_holdouts.json.gz').read_bytes()))['results']}
        closed=values['torus_crease_loop_0'];self.assertEqual(closed['before']['edge_tags'].count(2),8)
        self.assertEqual(closed['pack_count'],1)
        twisted=values['twisted_torus_0'];self.assertEqual(twisted['pack_count'],2)
        coincident=values['coincident_grids_0'];self.assertEqual(len(coincident['before']['points']),40)
        self.assertEqual(coincident['before']['points'][:20],coincident['before']['points'][20:])
        self.assertEqual(coincident['pack_count'],2)
        self.assertEqual(len(coincident['output']['geometry']['vertices']),8)

    def test_strict_allowlist_and_standalone_worker_import(self):
        query=json.loads((HERE/'fixtures/subd_packing.json').read_bytes())
        for change in [dict(mode=True),dict(mode=0.0),dict(mode=1),dict(scene='external'),dict(extra=True)]:
            op=copy.deepcopy(query['operations'][0]);op.update(change)
            with self.assertRaises(ValueError):validate_operations(dict(query,operations=[op]))
        with tempfile.TemporaryDirectory() as folder:
            stage_helpers(query,HERE,Path(folder))
            subprocess.run([sys.executable,'-I','-c',
                'import sys;sys.path.insert(0,sys.argv[1]);import subd_packing_probe',folder],check=True,capture_output=True)

if __name__=='__main__':unittest.main()
