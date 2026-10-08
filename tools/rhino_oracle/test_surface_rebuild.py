"""Closed surface Rebuild commands, source identity and immutable captures."""
import hashlib
import json
import os
from pathlib import Path
from unittest import TestCase, mock

from .client import OracleClient, OracleProtocolError
from .surface_rebuild_probe import request, run, validate_request

ROOT = Path(__file__).resolve().parents[2]


class SurfaceRebuildTests(TestCase):
    def test_closed_private_idle_recipes(self):
        q = request()
        validate_request(q)
        self.assertEqual(q, json.loads((ROOT / 'tools/rhino_oracle/fixtures/surface_rebuild.json').read_text()))
        for change in [dict(case='_Exit'), dict(id='x\n_Exit'), dict(extra=True), dict(op='script')]:
            with self.assertRaises(ValueError):
                validate_request(dict(q, operations=[dict(q['operations'][0], **change)]))
        rhino = mock.Mock()
        rhino.Commands.Command.InCommand.return_value = True
        with self.assertRaisesRegex(ValueError, 'idle execution'):
            run(q['operations'][0], dict(Rhino=rhino, System=mock.Mock()))
        rhino.Commands.Command.InCommand.return_value = False
        rhino.RhinoDoc.ActiveDoc.Objects = [object()]
        with self.assertRaisesRegex(ValueError, 'empty owned document'):
            run(q['operations'][0], dict(Rhino=rhino, System=mock.Mock()))
        with mock.patch.dict(os.environ, dict(DISPLAY=':301', VIBOCEROS_ORACLE_HEADLESS=':301'), clear=True), \
                mock.patch('tools.rhino_oracle.client._run_logged') as launch:
            with self.assertRaisesRegex(OracleProtocolError, 'private settings scheme'):
                OracleClient().run_rhino(q, 1)
            launch.assert_not_called()

    def test_complete_command_controls_history_and_provenance(self):
        p = json.loads((ROOT / 'docs/surface-rebuild-provenance.json').read_text())
        self.assertFalse(p['full_native_parity'])
        for name, digest in p['sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT / name).read_bytes()).hexdigest(), digest, name)
        q = json.loads((ROOT / 'tools/rhino_oracle/observations/surface_rebuild.json').read_text())
        self.assertEqual(len(q['results']), 12)
        self.assertEqual({r['id'] for r in q['results']}, {o['id'] for o in request()['operations']})
        for row in q['results']:
            v = row['value']
            self.assertTrue(v['command']['success'])
            self.assertFalse(v['command']['active'])
            self.assertEqual(v['undo']['after_script'], v['before'])
            self.assertEqual(v['redo']['after_script'], v['command']['after_script'])
            output = v['command']['after_script'][-1]
            self.assertEqual(output['definition'], v['rebuild_sdk'])
            self.assertEqual(output['definition']['control_count'], v['spec']['count'])
            self.assertEqual(output['definition']['degree'], v['spec']['degree'])
            self.assertEqual(output['source'], 0 if v['spec']['delete'] else None)
            self.assertEqual(output['layer'], 2 if v['spec']['current'] else 0)
            self.assertEqual(output['groups'], [])
            self.assertEqual(len(output['samples']), 81)

    def test_initial_ignored_option_diagnostic_keeps_default_outputs(self):
        q = json.loads((ROOT / 'tools/rhino_oracle/observations/surface_rebuild_initial.json').read_text())
        self.assertEqual(len(q['results']), 6)
        for row in q['results']:
            v = row['value']
            self.assertIn('_PointCount=', v['command']['macro'])
            self.assertEqual(v['command']['after_script'][-1]['definition']['control_count'], [10, 10])
            self.assertEqual(v['command']['after_script'][-1]['definition']['degree'], [3, 3])

    def test_sdk_capture_has_all_requested_target_structures(self):
        request = json.loads((ROOT / 'tools/rhino_oracle/fixtures/surface_rebuild_geometry.json').read_text())
        native = json.loads((ROOT / 'tools/rhino_oracle/observations/surface_rebuild_geometry.json').read_text())
        self.assertEqual(len(native['results']), 12)
        for op, row in zip(request['operations'], native['results']):
            self.assertEqual(op['id'], row['id'])
            surface = row['value']['surface']
            self.assertEqual(surface['degree'], op['degree'])
            self.assertEqual(surface['control_count'], op['point_count'])
            self.assertTrue(all(c['weight'] == 1. for c in surface['control_points']))

    def test_initial_local_audit_exposes_endpoint_rounding_without_geometry_mismatches(self):
        q = json.loads((ROOT / 'tools/rhino_oracle/observations/surface_rebuild_local_initial_audit.json').read_text())
        self.assertFalse(q['passed'])
        self.assertEqual((q['matched'], q['mismatched'], q['native_failed']), (10, 0, 2))
        failed = [r for r in q['operations'] if r['native_error']]
        self.assertEqual(len(failed), 2)
        self.assertTrue(all('arc-length distance' in r['native_error']['message'] for r in failed))
