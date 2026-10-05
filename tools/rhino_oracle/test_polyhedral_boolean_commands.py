"""Closed command recipes, isolated launches and retained compound countercases."""
import hashlib
import json
import os
from pathlib import Path
from unittest import TestCase, mock

from .client import OracleClient, OracleError, OracleProtocolError
from .polyhedral_command_probe import request, run, validate_request

ROOT = Path(__file__).resolve().parents[2]


class PolyhedralBooleanCommandTests(TestCase):
    def capture(self):
        return json.loads((ROOT/'tools/rhino_oracle/observations/polyhedral_boolean_command.json').read_text())

    def test_closed_recipes_owned_idle_document_and_private_launch(self):
        q = request()
        validate_request(q)
        self.assertEqual(q, json.loads((ROOT/'tools/rhino_oracle/fixtures/polyhedral_boolean_command.json').read_text()))
        for change in (dict(id='x\n_Delete'), dict(id=[]), dict(case=[]),
                       dict(case='_Exit'), dict(extra=True), dict(op='script')):
            with self.subTest(change=change), self.assertRaises(ValueError):
                validate_request(dict(q, operations=[dict(q['operations'][0], **change)]))
        for change in (dict(protocol_version=True), dict(iterations=True), dict(iterations=2),
                       dict(operations=[]), dict(operations=q['operations']*2),
                       dict(operations=[q['operations'][0]]*2)):
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

    def test_capture_retains_all_command_events_and_history(self):
        r = self.capture()
        self.assertEqual(r['engine_version'], '8.32.26160.13001')
        self.assertEqual(len(r['results']), 65)
        self.assertEqual([o['id'] for o in request()['operations']], [o['id'] for o in r['results']])
        for row in r['results']:
            c = row['value']['command']
            name = {'u': 'BooleanUnion', 'i': 'BooleanIntersection', 'd': 'BooleanDifference'}[row['id'][8]]
            ends = [e for e in c['events'] if e['name'] == name]
            self.assertEqual(len(ends), 1, row['id'])
            self.assertEqual(ends[0]['result'], 'Success', row['id'])
            self.assertEqual(ends[0]['objects'], c['after'], row['id'])
            self.assertTrue(c['success'], row['id'])
            self.assertNotIn('Unknown command:', c['history'], row['id'])
            self.assertTrue(all(g['valid'] and g['solid'] for g in row['value']['before']))
            for phase in ('undo', 'redo'):
                if phase in row['value']:
                    h = row['value'][phase]
                    events = [e for e in h['events'] if e['name'] == phase.title()]
                    self.assertEqual(len(events), 1)
                    self.assertEqual(events[0]['objects'], h['after'])
        non_solids = [r['id'] for r in r['results'] if any(not g['solid'] for g in r['value']['command']['after'])]
        self.assertEqual(non_solids, ['polycmd_i_singular_two_holes'])

    def test_compound_policies_and_unresolved_intersections_are_visible(self):
        rows = {r['id'].removeprefix('polycmd_'): r['value']['command']['after'] for r in self.capture()['results']}
        for case, volumes in (
            ('u_cavity', [28.75, .75]), ('i_cavity', [2.25, 3.75]),
            ('u_cavity_unopened', [27.75]), ('d_cavity_unopened', [26.75]),
            ('u_partial_multishell', [28.75]), ('d_partial_multishell', [24.75]),
            ('i_cavity_first_multi', [2.25, 3.75]), ('i_cavity_second_multi', [5.0625]),
        ):
            self.assertEqual(len(rows[case]), len(volumes), case)
            for native, volume in zip(rows[case], volumes):
                self.assertAlmostEqual(native['volume'], volume, places=10, msg=case)
                self.assertEqual(native['orientation'], 'Outward', case)
        self.assertEqual(rows['i_hole_three_sets'][0]['attribute_text'], 'attribute-0')
        self.assertEqual(rows['i_hole_three_sets'][0]['geometry_text'], 'geometry-2')
        for case in ('u_cavity', 'i_cavity', 'd_hole_split'):
            self.assertTrue(all(g['geometry_text'] is None for g in rows[case]))

    def test_capture_provenance(self):
        p = json.loads((ROOT/'docs/polyhedral-command-provenance.json').read_text())
        self.assertTrue(p['private_xvfb'])
        self.assertFalse(p['full_native_parity'])
        self.assertEqual(p['command_witnesses'], 65)
        self.assertEqual(p['physical_command_replays'], 62)
        self.assertEqual(p['output_partition_differences'], 7)
        partitions = json.loads((ROOT/'docs/polyhedral-command-partitions.json').read_text())
        self.assertEqual(len(partitions), 7)
        self.assertEqual(p['uncertified_recipes'], ['i_singular_two_holes', 'i_cavity_first_multi', 'i_cavity_second_multi'])
        for path, digest in p['sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(), digest, path)
