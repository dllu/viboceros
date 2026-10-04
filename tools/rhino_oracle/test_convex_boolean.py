"""Convex Boolean capture guards and immutable public SDK evidence."""
import hashlib
import json
import os
from pathlib import Path
from unittest import TestCase, mock

from .client import OracleClient, OracleError, OracleProtocolError
from .convex_boolean_probe import request, run, validate_request

ROOT = Path(__file__).resolve().parents[2]


class ConvexBooleanTests(TestCase):
    def test_closed_recipes_and_owned_document_guards(self):
        q = request()
        validate_request(q)
        self.assertEqual(q, json.loads((ROOT/'tools/rhino_oracle/fixtures/convex_boolean.json').read_text()))
        for change in (dict(id='x\n_Delete'), dict(id=[]), dict(case=[]),
                       dict(case='_Exit'), dict(operation=[]), dict(operation='_Delete'), dict(extra=True)):
            with self.subTest(change=change), self.assertRaises(ValueError):
                validate_request(dict(q, operations=[dict(q['operations'][0], **change)]))
        for change in (dict(protocol_version=True), dict(iterations=True), dict(iterations=2),
                       dict(operations=[]), dict(operations=q['operations']*2)):
            with self.subTest(change=change), self.assertRaises(ValueError):
                validate_request(dict(q, **change))
        rhino = mock.Mock()
        rhino.Commands.Command.InCommand.return_value = 1
        with self.assertRaisesRegex(ValueError, 'idle execution'):
            run(q['operations'][0], dict(Rhino=rhino))
        rhino.Commands.Command.InCommand.return_value = 0
        rhino.RhinoDoc.ActiveDoc.Objects = [object()]
        with self.assertRaisesRegex(ValueError, 'empty owned document'):
            run(q['operations'][0], dict(Rhino=rhino))

    def test_private_display_and_settings_precede_launch(self):
        for scheme, display, headless in ((None, ':301', ':301'),
                ('VibocerosOracleBooleans', ':1', None), ('VibocerosOracleBooleans', ':301', ':302')):
            env = dict(DISPLAY=display)
            if headless:
                env['VIBOCEROS_ORACLE_HEADLESS'] = headless
            with self.subTest(display=display, scheme=scheme), mock.patch.dict(os.environ, env, clear=True), \
                    mock.patch('tools.rhino_oracle.client._run_logged') as launch:
                with self.assertRaises((OracleError, OracleProtocolError, ValueError)):
                    OracleClient(settings_scheme=scheme).run_rhino(request(), 1)
                launch.assert_not_called()

    def test_capture_has_all_public_sdk_cases(self):
        r = json.loads((ROOT/'tools/rhino_oracle/observations/convex_boolean.json').read_text())
        self.assertEqual([o['id'] for o in request()['operations']], [o['id'] for o in r['results']])
        self.assertEqual(len(r['results']), 36)
        self.assertEqual(r['engine_version'], '8.32.26160.13001')
        for row in r['results']:
            for g in row['value']['inputs']+row['value']['outputs']:
                self.assertTrue(g['valid'])
                self.assertTrue(g['solid'])

    def test_provenance(self):
        p = json.loads((ROOT/'docs/convex-boolean-provenance.json').read_text())
        self.assertEqual(p['sdk_witnesses'], 36)
        self.assertTrue(p['private_xvfb'])
        self.assertFalse(p['full_native_parity'])
        self.assertFalse(p['commands_registered'])
        for path, expected in p['sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(), expected, path)
