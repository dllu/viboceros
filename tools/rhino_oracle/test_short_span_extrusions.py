"""Immutable public-command evidence for rational microscopic spans."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from .short_span_extrusions_cases import cases
from .structural_sdk import stage_helpers,validate_operations
from .subd_edge_analysis_replay import HERE,read


class ShortSpanExtrusionTests(unittest.TestCase):
    def test_fixed_requests_and_frozen_native_sources(self):
        path=HERE/'fixtures/short_span_extrusions.json';query=json.loads(path.read_bytes());data=read('short_span_extrusions')
        self.assertEqual(query['operations'],cases());validate_operations(query)
        self.assertEqual([r['id']for r in data['results']],[op['id']for op in cases()])
        self.assertEqual(data['provenance']['request_sha256'],hashlib.sha256(path.read_bytes()).hexdigest())
        for name,digest in data['provenance']['sources'].items():
            raw=gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(raw).hexdigest(),digest,name)

    def test_complete_native_geometry_selection_and_history(self):
        for op,row in zip(cases(),read('short_span_extrusions')['results'],strict=True):
            v=row['value'];self.assertTrue(v['success']);self.assertNotIn('Unknown command:',v['history'])
            self.assertEqual(v['undo'],v['before']);self.assertEqual(v['redo'],v['after']);self.assertEqual(v['after'][0],v['before'][0])
            self.assertEqual([e['result']for e in v['events']],['Success'])
            output=v['after'][1];self.assertEqual((output['kind'],output['selected'],output['faces'],output['caps']),('extrusion',0,2,[False,False]))
            surface=output['surface'];self.assertEqual(surface['degree'],[2,1]);self.assertEqual(surface['control_count'],[7,2])
            self.assertEqual(surface['knots_u'],op['curve']['knots'])
            axis=1 if op['id'].startswith('xz_')else 2
            for i,control in enumerate(surface['control_points']):
                original=op['curve']['control_points'][i%7];expected=list(original['point'])
                expected[axis]+=op['height']if i>=7 else -op['height']if op['both_sides']else 0.
                self.assertEqual(control['weight'],original['weight'])
                for actual,expected in zip(control['point'],expected):self.assertAlmostEqual(actual,expected,delta=1e-12)

    def test_malformed_and_non_owned_requests_rejected(self):
        for edit in (dict(extra=True),dict(id='unowned'),dict(height=True),dict(both_sides=1)):
            with self.assertRaises(ValueError):validate_operations(dict(operations=[dict(cases()[0],**edit)]))
        for location in ('degree','weight','knot','point'):
            op=copy.deepcopy(cases()[0]);curve=op['curve']
            if location=='degree':curve['degree']=2.
            elif location=='weight':curve['control_points'][0]['weight']=True
            elif location=='knot':curve['knots'][0]=False
            else:curve['control_points'][0]['point'][1]=False
            with self.assertRaises(ValueError):validate_operations(dict(operations=[op]))

    def test_isolated_public_sdk_helper_staging(self):
        query=dict(protocol_version=1,iterations=1,operations=cases())
        with tempfile.TemporaryDirectory()as directory:
            stage_helpers(query,HERE,Path(directory))
            subprocess.run([sys.executable,'-I','-c','import sys;sys.path.insert(0,sys.argv[1]);import short_span_extrusions_probe',directory],check=True,capture_output=True)


if __name__=='__main__':unittest.main()
