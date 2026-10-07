"""Closed native ApplyCrv workflows and independent local input fixtures."""
import hashlib
import json
import os
from pathlib import Path
from unittest import TestCase, mock

from .apply_uv_curves_probe import request, run, validate_request
from .client import OracleClient, OracleError, OracleProtocolError

ROOT = Path(__file__).resolve().parents[2]


def read(path):
    return json.loads((ROOT / path).read_text())


class ApplyUvCurvesTests(TestCase):
    def test_closed_recipes_require_idle_empty_owned_document_and_private_display(self):
        q = request()
        validate_request(q)
        self.assertEqual(q, read('tools/rhino_oracle/fixtures/apply_uv_curves_command.json'))
        for change in (dict(id='x\n_Exit'), dict(id=[]), dict(case=[]), dict(case='_Exit'),
                       dict(extra=True), dict(op='script')):
            with self.subTest(change=change), self.assertRaises(ValueError):
                validate_request(dict(q, operations=[dict(q['operations'][0], **change)]))
        for change in (dict(protocol_version=True), dict(iterations=True), dict(iterations=2),
                       dict(operations=[]), dict(operations=q['operations'] * 3)):
            with self.subTest(change=change), self.assertRaises(ValueError):
                validate_request(dict(q, **change))
        rhino = mock.Mock()
        rhino.Commands.Command.InCommand.return_value = 1
        with self.assertRaisesRegex(ValueError, 'idle execution'):
            run(q['operations'][0], dict(Rhino=rhino, System=mock.Mock()))
        rhino.Commands.Command.InCommand.return_value = 0
        rhino.RhinoDoc.ActiveDoc.Objects = [object()]
        with self.assertRaisesRegex(ValueError, 'empty owned document'):
            run(q['operations'][0], dict(Rhino=rhino, System=mock.Mock()))
        for scheme, env in (
            (None, dict(DISPLAY=':301', VIBOCEROS_ORACLE_HEADLESS=':301')),
            ('VibocerosOracleTest', dict(DISPLAY=':1')),
            ('VibocerosOracleTest', dict(DISPLAY=':301', VIBOCEROS_ORACLE_HEADLESS=':302')),
        ):
            with mock.patch.dict(os.environ, env, clear=True), mock.patch('tools.rhino_oracle.client._run_logged') as launch:
                with self.assertRaises((OracleError, OracleProtocolError, ValueError)):
                    OracleClient(settings_scheme=scheme).run_rhino(q, 1)
                launch.assert_not_called()

    def test_native_capture_keeps_original_source_geometry_and_independent_history(self):
        native = read('tools/rhino_oracle/observations/apply_uv_curves_command.json')
        self.assertEqual(native['engine_version'], '8.32.26160.13001')
        self.assertEqual([r['id'] for r in native['results']], [op['id'] for op in request()['operations']])
        curves = points = 0
        for row in native['results']:
            v = row['value']
            self.assertTrue(v['success'])
            self.assertFalse(v['command_active'])
            self.assertTrue(v['history_cleared_before_command'])
            originals = {o['source']: o for o in v['before']}
            for o in v['after']:
                if o['source'] is not None:
                    expected = dict(originals[o['source']], selected=o['selected'])
                    self.assertEqual(o, expected, row['id'])
                else:
                    self.assertTrue(o['selected'])
                    self.assertTrue(o['layer'].endswith('_input'))
                    self.assertEqual(o['user_text'], o['name'])
                    if o['kind'] == 'curve':
                        curves += 1
                        self.assertEqual(len(o['samples']), 33)
                    else:
                        points += 1
                        self.assertEqual(o['groups'], [])
            self.assertEqual(v['groups_after'], v['groups_undo'])
            self.assertEqual(v['groups_after'], v['groups_redo'])
        self.assertEqual((curves, points), (17, 9))

    def test_degenerate_native_mapping_has_no_undo_entry_and_empty_redo_selection(self):
        rows = read('tools/rhino_oracle/observations/apply_uv_curves_command.json')['results']
        for row in rows:
            v = row['value']
            if row['id'].endswith(('horizontal', 'single_point')):
                self.assertIn('Nothing to undo.', v['undo_history'])
                self.assertEqual(len(v['before']), len(v['after']))
                self.assertTrue(all(not o['selected'] for o in v['undo'] + v['redo']))
            else:
                self.assertIn('Undoing ApplyCrv', v['undo_history'])

    def test_local_fixture_keeps_exact_native_inputs_and_local_history_contract(self):
        native = read('tools/rhino_oracle/observations/apply_uv_curves_command.json')['results']
        fixture = read('tools/rhino_oracle/fixtures/apply_uv_curves_local.json')['operations']
        local = read('docs/apply-uv-curves-local.json')['results']
        self.assertEqual(len(native), len(fixture))
        self.assertEqual(len(native), len(local))
        for n, f, l in zip(native, fixture, local):
            self.assertEqual(n['id'], f['id'])
            self.assertEqual(l['id'], f['id'])
            self.assertEqual(f['inputs'], n['value']['inputs'])
            self.assertEqual(f['surface']['control_points'], n['value']['surface']['control_points'])
            for field in ('knots_u', 'knots_v', 'domain_u', 'domain_v'):
                self.assertEqual(f['surface'][field], n['value']['surface'][field])
            count = sum(o['source'] is None for o in n['value']['after'])
            value = l['value']
            self.assertEqual(sum(o['source'] is None for o in value['after']), count)
            self.assertEqual(value['undoable'], count != 0)
            self.assertEqual(value['groups_after'], value['groups_after_undo'])
            self.assertTrue(all(not o['selected'] for o in value['undo']))
            self.assertEqual(sum(o['selected'] for o in value['redo']), count)

    def test_provenance_preserves_source_hashes_and_limits(self):
        p = read('docs/apply-uv-curves-provenance.json')
        self.assertTrue(p['private_xvfb'])
        self.assertFalse(p['full_native_parity'])
        self.assertEqual((p['recipes'], p['created_curves'], p['created_points']), (13, 17, 9))
        self.assertEqual(p['settings_scheme'], 'VibocerosOracleApplyUVPurity20261006')
        for path, digest in p['sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT / path).read_bytes()).hexdigest(), digest, path)
