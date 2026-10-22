"""Fixed mesh assets, capture admission, and complete channel qualification."""
import base64
import copy
import gzip
import hashlib
import json
import os
from pathlib import Path
import unittest
from unittest.mock import patch

from . import mesh_channels_probe as probe
from .client import OracleClient, OracleProtocolError
from .structural_sdk import validate_operations

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]


class MeshChannelsTests(unittest.TestCase):
    def test_original_capture_and_saved_producers_are_complete(self):
        query_path = HERE/'fixtures/mesh_channels.json'
        query = json.loads(query_path.read_text())
        self.assertEqual(query, probe.request())
        validate_operations(query)
        result = json.loads((HERE/'observations/mesh_channels.json').read_text())
        self.assertEqual(result['provenance']['request_sha256'], hashlib.sha256(query_path.read_bytes()).hexdigest())
        self.assertEqual(len(result['results']), 28)
        for op, row in zip(query['operations'], result['results']):
            self.assertEqual(row['id'], op['id'])
            self.assertNotIn('error', row)
            self.assertEqual(row['value']['input'], probe.CASES[op['case']])
        for path, digest in result['provenance']['sources'].items():
            data = gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(data).hexdigest(), digest, path)

    def test_assets_are_exact_original_native_three_dm_files(self):
        assets = json.loads((HERE/'fixtures/mesh_channel_assets.json').read_text())
        self.assertEqual(set(assets), {'texture_cube', 'ngon'})
        for asset in assets.values():
            data = base64.b64decode(asset['data'], validate=True)
            self.assertEqual(data, (ROOT/asset['path']).read_bytes())
            self.assertEqual(hashlib.sha256(data).hexdigest(), asset['sha256'])

    def test_capture_rejects_unowned_mutated_and_repeated_operations(self):
        for fields in [{'id': 'arbitrary'}, {'case': []}, {'script': 'arbitrary'}]:
            query = probe.request()
            query['operations'][0].update(fields)
            with self.assertRaises(ValueError):
                validate_operations(query)
        with patch.dict(os.environ, {'DISPLAY': ':999', 'VIBOCEROS_ORACLE_HEADLESS': ':999'}), patch('subprocess.Popen') as spawn:
            with self.assertRaises(OracleProtocolError):
                OracleClient().run_rhino(probe.request())
            client = OracleClient(settings_scheme='VibocerosOracleMeshChannelsTest20261022')
            for kind in ['repeat', 'boolean', 'duplicate', 'count']:
                query = probe.request()
                if kind == 'repeat':
                    query['iterations'] = 2
                elif kind == 'boolean':
                    query['iterations'] = True
                elif kind == 'duplicate':
                    query['operations'].append(copy.deepcopy(query['operations'][0]))
                else:
                    query['operations'] *= 3
                with self.assertRaises(OracleProtocolError):
                    client.run_rhino(query)
            spawn.assert_not_called()

    def test_report_requires_every_comparison_and_retains_finite_scope(self):
        from .report_mesh_channels import summarize
        report = json.loads((ROOT/'docs/mesh-channels-qualification.json').read_text())
        lines = ['MESH_CHANNEL_NATIVE '+json.dumps(r) for r in report['cases']]
        self.assertEqual(summarize('\n'.join(lines)), report)
        self.assertEqual((report['compared_cases'], report['qualified_cases']), (28, 28))
        self.assertTrue(report['captured_cases_qualified'])
        self.assertFalse(report['full_native_parity'])
        self.assertFalse(report['performance_qualified'])
        for bad in [lines[:-1], lines+lines[:1]]:
            with self.assertRaises(ValueError):
                summarize('\n'.join(bad))
