"""Closed signed-shell witnesses, owned-document guards and retained evidence."""
import hashlib
import json
import os
from pathlib import Path
from unittest import TestCase, mock

from .client import OracleClient, OracleError, OracleProtocolError
from .compound_pairs_probe import request, run, validate_request

ROOT = Path(__file__).resolve().parents[2]


class CompoundPairsTests(TestCase):
    def capture(self):
        return json.loads((ROOT/'tools/rhino_oracle/observations/compound_pairs.json').read_text())

    def test_closed_recipes_require_idle_owned_document_and_private_display(self):
        q = request()
        validate_request(q)
        self.assertEqual(q, json.loads((ROOT/'tools/rhino_oracle/fixtures/compound_pairs.json').read_text()))
        for change in (dict(id='x\n_Exit'), dict(id=[]), dict(case=[]), dict(case='_Exit'),
                       dict(extra=True), dict(op='script')):
            with self.subTest(change=change), self.assertRaises(ValueError):
                validate_request(dict(q, operations=[dict(q['operations'][0], **change)]))
        for change in (dict(protocol_version=True), dict(iterations=True), dict(iterations=2),
                       dict(operations=[]), dict(operations=q['operations']*3),
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

    def test_capture_retains_events_winding_and_signed_sdk_outcomes(self):
        capture = self.capture()
        self.assertEqual(capture['engine_version'], '8.32.26160.13001')
        self.assertEqual([r['id'] for r in capture['results']], [r['id'] for r in request()['operations']])
        sdk, commands, histories, open_boundaries = 0, 0, 0, 0
        for row in capture['results']:
            value = row['value']
            if row['id'].startswith('pairs_sdk_'):
                sdk += 1
                self.assertEqual(len(value['inputs']), 2)
                geometry = value['inputs'] + value['outputs']
            else:
                commands += 1
                command = value['command']
                ends = [e for e in command['events'] if e['name'] == 'BooleanIntersection']
                self.assertEqual(len(ends), 1, row['id'])
                self.assertIn(ends[0]['result'], ('Success', 'Failure', 'Nothing'))
                self.assertEqual(ends[0]['objects'], command['after'])
                self.assertNotIn('Unknown command:', command['history'])
                geometry = value['before'] + command['after']
                for phase in ('undo', 'redo'):
                    if phase in value:
                        histories += 1
                        history = value[phase]
                        events = [e for e in history['events'] if e['name'] == phase.title()]
                        self.assertEqual(len(events), 1)
                        self.assertEqual(events[0]['objects'], history['after'])
                        geometry += history['after']
            for g in geometry:
                self.assertTrue(g['valid'], row['id'])
                self.assertEqual(len(g['face_reversed']), g['faces'])
                self.assertEqual(len(g['face_regions']), g['faces'])
                if not g['solid']:
                    open_boundaries += 1
                    self.assertIsNone(g['volume'])
                    self.assertIsNone(g['centroid'])
                    self.assertEqual((g['faces'], g['edges']), (12, 23))
        # SDK edge contact and both pair/common command edge contacts.
        self.assertEqual((sdk, commands, histories, open_boundaries), (8, 46, 8, 3))
        rows = {r['id'].removeprefix('pairs_'): r['value'] for r in capture['results']}
        self.assertTrue(rows['sdk_equal']['returned_null'])
        for case in ('nested', 'point', 'disjoint'):
            self.assertFalse(rows['sdk_'+case]['returned_null'])
            self.assertEqual(rows['sdk_'+case]['outputs'], [])
        g = rows['sdk_crossing']['outputs'][0]
        self.assertEqual(g['orientation'], 'Inward')
        self.assertAlmostEqual(g['volume'], -1.875)

    def test_common_participation_and_pair_metadata_are_distinct(self):
        rows = {r['id'].removeprefix('pairs_'): r['value'] for r in self.capture()['results']}
        after = lambda case: rows[case]['command']['after']
        for case, volumes in (
            ('pair_crossing', [15.625, 1.875]),
            ('pair_nested', [15.625, 1.]),
            ('common_cross', [2.]),
            ('common_cavities_nested', [15.625]),
            ('common_cavities_contained_outer', [1.875]),
            ('common_three_cavities', [2.078125]),
        ):
            self.assertEqual(len(after(case)), len(volumes), case)
            for g, volume in zip(after(case), volumes):
                self.assertAlmostEqual(g['volume'], volume, places=10, msg=case)
        self.assertEqual([g['name'] for g in after('pair_nested_reverse')], ['source-1', 'source-0'])
        for case in ('common_three_enclosing_nested', 'common_three_enclosing_disjoint', 'common_three_touch'):
            self.assertEqual(after(case)[0]['geometry_text'], 'geometry-1')
        self.assertTrue(all(g['name'] == 'source-0' for g in after('common_three_enclosing_first')))
        g = after('common_touches_inner_reverse')[0]
        self.assertEqual((g['name'], g['geometry_text']), ('source-1', 'geometry-0'))
        self.assertTrue(all(g['selected'] for g in after('pair_crossing_pre')))
        self.assertTrue(all(not g['selected'] for g in after('common_cross_pre')))

    def test_capture_provenance_and_explicit_partition_differences(self):
        p = json.loads((ROOT/'docs/compound-pairs-provenance.json').read_text())
        self.assertTrue(p['private_xvfb'])
        self.assertFalse(p['full_native_parity'])
        self.assertEqual(p['settings_scheme'], 'VibocerosOracleCompoundPairsOrientedFinal_20261004')
        self.assertEqual((p['sdk_pair_witnesses'], p['command_witnesses'], p['physical_command_records']), (8, 46, 46))
        self.assertEqual((p['previous_capture_physical_replays'], p['previous_capture_non_solid_outputs']), (65, 1))
        partitions = json.loads((ROOT/'docs/compound-pairs-partitions.json').read_text())
        self.assertEqual(len(partitions), p['output_partition_differences'])
        for path, digest in p['sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(), digest, path)
