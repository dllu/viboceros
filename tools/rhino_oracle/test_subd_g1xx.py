"""Immutable native G1xx controls, including quartic and mixed-degree outputs."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from . import subd_g1xx_probe as probe
from .structural_sdk import stage_helpers,validate_operations

HERE=Path(__file__).resolve().parent

class SubDG1xxTests(unittest.TestCase):
    def test_fixed_requests_frozen_sources_and_valid_immutable_outputs(self):
        for family,group,count in [('subd_g1xx','base',28),('subd_g1xx_even','even',7),
                                     ('subd_g1xx_charts','charts',4),
                                     ('subd_g1xx_creases','creases',2),
                                     ('subd_g1xx_degree','degree',2)]:
            path=HERE/'fixtures'/(family+'.json');query=json.loads(path.read_bytes())
            data=json.loads(gzip.decompress((HERE/'observations'/(family+'.json.gz')).read_bytes()))
            self.assertEqual(query['operations'],probe.cases(group));validate_operations(query)
            self.assertEqual(len(data['results']),count)
            self.assertEqual([r['id'] for r in data['results']],[o['id'] for o in query['operations']])
            self.assertEqual(data['provenance']['request_sha256'],hashlib.sha256(path.read_bytes()).hexdigest())
            for name,digest in data['provenance']['sources'].items():
                raw=gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(raw).hexdigest(),digest,name)
            for row in data['results']:
                value=row['value'];self.assertIs(value['source_unchanged'],True)
                self.assertIs(value['before']['valid'],True);self.assertIs(value['output']['valid'],True)

    def test_even_valence_outputs_include_quartics_and_mixed_degrees(self):
        load=lambda family:{r['id']:r['value'] for r in json.loads(gzip.decompress(
            (HERE/'observations'/(family+'.json.gz')).read_bytes()))['results']}
        values=load('subd_g1xx');values.update(load('subd_g1xx_even'))
        values.update(load('subd_g1xx_charts'));values.update(load('subd_g1xx_creases'));values.update(load('subd_g1xx_degree'))
        degrees=lambda key:{tuple(f['definition']['degree']) for f in values[key]['output']['geometry']['faces']}
        self.assertEqual(degrees('fan6_4'),{(4,4)})
        self.assertEqual(degrees('fan9_4'),{(3,3)})
        self.assertEqual(degrees('trapezo6_4'),{(4,4)})
        self.assertEqual(degrees('trapezo6_charts_4'),{(3,3),(4,4)})
        self.assertEqual(degrees('even_corner1_4'),{(4,4)})
        self.assertEqual(degrees('dart8_4'),{(3,3)})
        self.assertEqual(values['dart8_4']['before']['vertex_tags'][0],4)
        self.assertEqual(values['trapezo6_first_crease_4']['output']['geometry']['faces'][0]['definition']['degree'],[4,4])
        # Dart controls are unchanged, but their odd valence still selects
        # cubic degree ahead of the smooth even corner.
        for key in ('trapezo6_first_even1_4','trapezo6_first_even2_4'):
            self.assertEqual(values[key]['output']['geometry']['faces'][0]['definition']['degree'],[3,3])

    def test_strict_allowlist_and_standalone_import(self):
        query=json.loads((HERE/'fixtures/subd_g1xx.json').read_bytes())
        for change in [dict(mode=True),dict(mode=4.0),dict(mode=3),dict(scene='external'),dict(extra=True)]:
            op=copy.deepcopy(probe.cases()[0]);op.update(change)
            with self.assertRaises(ValueError):validate_operations(dict(query,operations=[op]))
        with tempfile.TemporaryDirectory() as folder:
            stage_helpers(query,HERE,Path(folder))
            subprocess.run([sys.executable,'-I','-c',
                'import sys;sys.path.insert(0,sys.argv[1]);import subd_g1xx_probe',folder],check=True,capture_output=True)

if __name__=='__main__':unittest.main()
