"""Closed recipes, isolation guards, immutable captures and semantic diagnostics."""
import copy
import hashlib
import json
import os
from pathlib import Path
from unittest import TestCase, mock

from .client import OracleClient, OracleError, OracleProtocolError
from .polyhedral_boolean_probe import request, run, validate_request
from .polyhedral_boolean import compare_polyhedral_responses

ROOT = Path(__file__).resolve().parents[2]


class PolyhedralBooleanTests(TestCase):
    def capture(self):
        return json.loads((ROOT/'tools/rhino_oracle/observations/polyhedral_boolean.json').read_text())

    def test_closed_recipes_idle_empty_document_and_private_launch_guards(self):
        q = request()
        validate_request(q)
        self.assertEqual(q, json.loads((ROOT/'tools/rhino_oracle/fixtures/polyhedral_boolean.json').read_text()))
        for change in (dict(id='x\n_Delete'), dict(id=[]), dict(case=[]), dict(operation=[]),
                       dict(case='_Exit'), dict(extra=True)):
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
        for scheme, env in ((None, dict(DISPLAY=':301', VIBOCEROS_ORACLE_HEADLESS=':301')),
                           ('VibocerosOracleTest', dict(DISPLAY=':1'))):
            with mock.patch.dict(os.environ, env, clear=True), mock.patch('tools.rhino_oracle.client._run_logged') as launch:
                with self.assertRaises((OracleError, OracleProtocolError, ValueError)):
                    OracleClient(settings_scheme=scheme).run_rhino(q, 1)
                launch.assert_not_called()

    def test_capture_retains_native_open_and_orientation_dependent_outcomes(self):
        r = self.capture()
        self.assertEqual([o['id'] for o in request()['operations']], [o['id'] for o in r['results']])
        self.assertEqual(r['engine_version'], '8.32.26160.13001')
        self.assertEqual(len(r['results']), 30)
        non_solids = []
        for row in r['results']:
            self.assertFalse(row['value']['returned_null'])
            self.assertTrue(all(g['valid'] and g['solid'] for g in row['value']['inputs']))
            if any(not g['solid'] for g in row['value']['outputs']):
                non_solids.append(row['id'])
        self.assertEqual(non_solids, ['polyhedral_singular_two_holes_intersection'])
        rows = {r['id']: r['value'] for r in r['results']}
        self.assertLess(rows['polyhedral_reversed_hole_union']['outputs'][0]['volume'], 0)
        self.assertLess(rows['polyhedral_cavity_union']['outputs'][1]['volume'], 0)

    def test_comparison_checks_mass_bounds_boundaries_and_singular_status(self):
        r = self.capture()
        own = copy.deepcopy(r)
        own['engine'] = 'viboceros'
        rows = compare_polyhedral_responses(own, r)
        self.assertEqual([x['id'] for x in rows if not x['passed']], ['polyhedral_singular_two_holes_intersection'])
        own['results'][0]['value']['outputs'][0]['volume'] += .1
        self.assertTrue(any('volume differs' in x for x in compare_polyhedral_responses(own, r)[0]['differences']))
        own = copy.deepcopy(r)
        own['engine'] = 'viboceros'
        own['results'][0]['value']['outputs'][0]['vertices'][0] = [999., 999., 999.]
        self.assertTrue(any('boundary witness differs' in x for x in compare_polyhedral_responses(own, r)[0]['differences']))
        for value in (0, -1, True, float('nan'), float('inf')):
            with self.assertRaises(ValueError):
                compare_polyhedral_responses(own, r, boundary_epsilon=value)
        own['results'][0]['value']['outputs'][0]['volume'] = float('nan')
        with self.assertRaises(OracleProtocolError):
            compare_polyhedral_responses(own, r)

    def test_provenance(self):
        p = json.loads((ROOT/'docs/polyhedral-boolean-provenance.json').read_text())
        self.assertTrue(p['private_xvfb'])
        self.assertFalse(p['full_native_parity'])
        self.assertEqual(p['sdk_witnesses'], 30)
        for path, expected in p['sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(), expected, path)
