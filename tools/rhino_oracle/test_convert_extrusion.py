"""Immutable native B-rep conversions, shared files and bounded SDK staging."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from .convert_extrusion_cases import ASSETS,cases,pilot_cases
from .convert_extrusion_replay import FAMILIES
from .structural_sdk import stage_helpers,validate_operations
from .subd_edge_analysis_replay import HERE,read


class ConvertExtrusionTests(unittest.TestCase):
    def test_frozen_requests_assets_and_producers(self):
        for (family,size),recipes in zip(FAMILIES,(pilot_cases(),cases()),strict=True):
            path=HERE/('fixtures/'+family+'.json');request=json.loads(path.read_bytes())
            validate_operations(request);self.assertEqual(request['operations'],recipes)
            data=read(family);self.assertEqual(len(data['results']),size)
            self.assertEqual([r['id'] for r in data['results']],[op['id'] for op in recipes])
            self.assertEqual(data['provenance']['request_sha256'],hashlib.sha256(path.read_bytes()).hexdigest())
            for name,digest in data['provenance']['sources'].items():
                raw=gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(raw).hexdigest(),digest,name)
                if name.endswith('.3dm'):self.assertEqual((HERE/name).read_bytes(),raw)

    def test_native_conversion_retention_metadata_and_history(self):
        for (family,size),recipes in zip(FAMILIES,(pilot_cases(),cases()),strict=True):
            for op,row in zip(recipes,read(family)['results'],strict=True):
                v=row['value'];self.assertTrue(v['success'],row['id'])
                self.assertNotIn('Unknown command:',v['history'])
                self.assertEqual(v['undo'],v['before'],row['id'])
                self.assertEqual(v['redo'],v['after_script'],row['id'])
                deleting=op['delete'] is not False
                output=[o for o in v['after_script']['objects'] if o['kind']=='brep']
                self.assertEqual(len(output),2 if op.get('pair') else 1)
                self.assertTrue(all(o['role'].startswith('source' if deleting else 'copy') for o in output))
                self.assertTrue(all(o['geometry_text'] is None and o['attribute_text']=='Input extrusion' for o in output))
                self.assertTrue(all(o['selected']==0 for o in v['after_script']['objects']) if not op['preselect'] else True)
                if not deleting:
                    self.assertTrue(all(o['selected']==0 for o in output))
                self.assertEqual([o for o in v['before']['objects'] if o['kind']=='point'],[o for o in v['after_script']['objects'] if o['kind']=='point'])

    def test_input_validation_and_isolated_transitive_staging(self):
        for op in pilot_cases()+cases():
            for change in (dict(asset='../outside'),dict(delete=1),dict(preselect=0),dict(grouped=1),dict(pair=1),dict(reverse=1),dict(extra=True)):
                with self.assertRaises(ValueError):validate_operations(dict(operations=[dict(copy.deepcopy(op),**change)]))
        request=dict(protocol_version=1,iterations=1,operations=pilot_cases()+cases())
        with tempfile.TemporaryDirectory() as folder:
            stage_helpers(request,HERE,Path(folder))
            for name in ASSETS:
                path=Path('fixtures/extrusions')/(name+'.3dm')
                self.assertEqual((Path(folder)/path).read_bytes(),(HERE/path).read_bytes())
            subprocess.run([sys.executable,'-I','-c','import sys;sys.path.insert(0,sys.argv[1]);import convert_extrusion_probe',folder],check=True,capture_output=True)


if __name__=='__main__':unittest.main()
