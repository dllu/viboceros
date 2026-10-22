"""Fixed input, original producer and native command admission checks."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from . import control_polygon_cases as cases
from .structural_sdk import stage_helpers, validate_operations

HERE = Path(__file__).resolve().parent


class ControlPolygonTests(unittest.TestCase):
    def test_owned_capture_rejects_unbounded_or_mixed_requests_before_launch(self):
        from .client import OracleClient, OracleProtocolError
        query = cases.request()
        invalid = []
        for iterations in [True,2]:
            invalid.append(dict(copy.deepcopy(query),iterations=iterations))
        duplicate = copy.deepcopy(query)
        duplicate['operations'].append(duplicate['operations'][0])
        invalid.append(duplicate)
        mixed = copy.deepcopy(query)
        mixed['operations'].append(dict(op='line_length',id='other'))
        invalid.append(mixed)
        with patch.dict('os.environ',{'DISPLAY':':917','VIBOCEROS_ORACLE_HEADLESS':':917'}), \
                patch('subprocess.Popen') as launch:
            for request in invalid:
                with self.assertRaises((ValueError,OracleProtocolError)):
                    OracleClient(settings_scheme='VibocerosOracleControlPolygonTest').run_rhino(request)
            with self.assertRaises(OracleProtocolError):
                OracleClient().run_rhino(query)
            launch.assert_not_called()

    def test_complete_fixed_requests_and_frozen_native_producers(self):
        for suffix, request, count in [('', cases.request, 37), ('_options', cases.options_request, 12),
                                        ('_priority', cases.priority_request, 12)]:
            family = 'control_polygon'+suffix
            path = HERE/'fixtures'/(family+'.json')
            query = json.loads(path.read_text())
            self.assertEqual(query,request())
            validate_operations(query)
            native = json.loads((HERE/'observations'/(family+'.json')).read_text())
            self.assertEqual(native['engine_version'],'8.32.26160.13001')
            self.assertEqual(len(native['results']),count)
            self.assertEqual([r['id'] for r in native['results']],[o['id'] for o in query['operations']])
            self.assertEqual(native['provenance']['request_sha256'],hashlib.sha256(path.read_bytes()).hexdigest())
            for name,digest in native['provenance']['sources'].items():
                original = gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(original).hexdigest(),digest,name)
            for row in native['results']:
                self.assertNotIn('error',row)
                value = row['value']
                self.assertEqual(value['input'],cases.SCENES[row['id']])
                events = [e for e in value['events'] if e['name']=='ExtractControlPolygon']
                self.assertEqual(len(events),1,row['id'])
                before = {o['source']:o['crc'] for o in value['before'] if o['source'] is not None}
                after = {o['source']:o['crc'] for o in value['after'] if o['source'] is not None}
                self.assertEqual(before,after,row['id'])
                for stage in ['source','result']:
                    self.assertTrue((HERE/'fixtures'/family/stage/(row['id']+'.3dm')).is_file())

    def test_arbitrary_operations_and_unstaged_imports_are_rejected(self):
        query = cases.request()
        for change in [dict(case='eval'),dict(case=[]),dict(id='different'),dict(extra=True)]:
            bad = copy.deepcopy(query)
            bad['operations'][0].update(change)
            with self.assertRaises(ValueError):
                validate_operations(bad)
        with tempfile.TemporaryDirectory() as directory:
            stage_helpers(query,HERE,Path(directory))
            self.assertEqual({p.name for p in Path(directory).iterdir()},
                {'control_polygon_probe.py','control_polygon_cases.py','join_probe.py','merge_edges_probe.py'})
