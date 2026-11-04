"""Immutable public soft-crease inputs and full SDK output coverage."""
import base64
import copy
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from .subd_soft_nurbs_cases import cases,holdout_cases,even_cases,chart_cases,offset_cases,source as recipe_source,ASSETS
from .structural_sdk import stage_helpers,validate_operations

HERE=Path(__file__).resolve().parent
class SubDSoftNurbsTests(unittest.TestCase):
    def test_frozen_inputs_sources_valid_outputs_and_immutable_native_queries(self):
        for name,recipes,count in [('subd_soft_nurbs',cases(),41),('subd_soft_nurbs_holdouts',holdout_cases(),36),('subd_soft_nurbs_even',even_cases(),30),('subd_soft_nurbs_charts',chart_cases(),50),('subd_soft_nurbs_offsets',offset_cases(),18)]:
            path=HERE/'fixtures'/(name+'.json');query=json.loads(path.read_bytes())
            data=json.loads(gzip.decompress((HERE/'observations'/(name+'.json.gz')).read_bytes()))
            self.assertEqual(query['operations'],recipes);validate_operations(query)
            self.assertEqual(len(data['results']),count)
            self.assertEqual([r['id'] for r in data['results']],[o['id'] for o in recipes])
            self.assertEqual(data['provenance']['request_sha256'],hashlib.sha256(path.read_bytes()).hexdigest())
            for source,digest in data['provenance']['sources'].items():
                raw=gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(raw).hexdigest(),digest,source)
            for row in data['results']:
                v=row['value'];self.assertIs(v['source_unchanged'],True);self.assertIs(v['before']['valid'],True)
                if v['soft_results'] is not None:self.assertEqual(v['soft_results'],['Success'])
                tiles=sum(1 if len(f)==4 else len(f) for f in v['before']['faces'])
                self.assertEqual(len(v['unpacked']['geometry']['faces']),tiles)
                for kind in ('unpacked','output'):
                    self.assertIs(v[kind]['valid'],True)
                    self.assertEqual(len(v[kind]['pack_ids']),len(v[kind]['geometry']['faces']))
                    self.assertTrue(all(i>0 for i in v[kind]['pack_ids']))
            if name in ('subd_soft_nurbs_charts','subd_soft_nurbs_offsets'):
                for op,row in zip(recipes,data['results']):
                    actual=row['value']['input'];expected=recipe_source(op)
                    self.assertEqual(actual['faces'],expected['faces'])
                    self.assertEqual(actual.get('corners'),expected.get('corners'))
                    self.assertEqual(len(actual['points']),len(expected['points']))
                    # Native and local Python coordinates can differ by roundoff.
                    self.assertTrue(all(abs(a-b)<1e-14 for p,q in zip(actual['points'],expected['points']) for a,b in zip(p,q)))
                    self.assertEqual(row['value']['soft_edge'],op.get('edge',[0,1]))

    def test_variable_inputs_are_exact_archives_from_successful_native_commands(self):
        folder=HERE/'fixtures/subd_soft_nurbs_inputs';manifest=json.loads((folder/'provenance.json').read_bytes())
        self.assertEqual(set(manifest),set(ASSETS))
        for name,entry in manifest.items():
            path=HERE/'observations'/(entry['family']+'.json');raw=path.read_bytes()
            self.assertEqual(hashlib.sha256(raw).hexdigest(),entry['source_sha256'])
            row=next(r for r in json.loads(raw)['results'] if r['id']==entry['case'])
            self.assertIs(row['value']['success'],True)
            archive=base64.b64decode(row['value']['result_3dm'])
            self.assertEqual((folder/(name+'.3dm')).read_bytes(),archive)
            self.assertEqual(hashlib.sha256(archive).hexdigest(),entry['archive_sha256'])

    def test_fixed_dispatch_types_and_standalone_worker_import(self):
        query=json.loads((HERE/'fixtures/subd_soft_nurbs.json').read_bytes())
        for change in [dict(weight=10),dict(mode=True),dict(scene='external'),dict(extra=True)]:
            op=copy.deepcopy(query['operations'][0]);op.update(change)
            with self.assertRaises(ValueError):validate_operations(dict(query,operations=[op]))
        for edge in ([False,3],[0.,3],[0,4]):
            op=copy.deepcopy(offset_cases()[0]);op['edge']=edge
            with self.assertRaises(ValueError):validate_operations(dict(query,operations=[op]))
        with tempfile.TemporaryDirectory() as folder:
            stage_helpers(query,HERE,Path(folder))
            subprocess.run([sys.executable,'-I','-c','import sys;sys.path.insert(0,sys.argv[1]);import subd_soft_nurbs_probe',folder],check=True,capture_output=True)

if __name__=='__main__':unittest.main()
