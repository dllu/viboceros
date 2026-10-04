"""Bounded private ScaleNU recipes and retained native compatibility gaps."""
import hashlib
import json
import os
from pathlib import Path
from unittest import TestCase, mock

from .client import OracleClient, OracleError, OracleProtocolError
from .scale_nu_probe import request, run, validate

ROOT = Path(__file__).resolve().parents[2]


def read(folder):
    return json.loads((ROOT / 'tools/rhino_oracle' / folder / 'scale_nu.json').read_text())


class ScaleNuTests(TestCase):
    def test_closed_recipes_owned_document_and_selection_rules(self):
        q = request()
        self.assertEqual(q, read('fixtures'))
        for op in q['operations']:
            validate(op)
        for change in (dict(extra=True), dict(id='x\n_Delete'), dict(id='λ'),
                       dict(id=[]), dict(source=[]), dict(source='anything'),
                       dict(plane=[]), dict(input=[]), dict(input='numeric _Exit'),
                       dict(copy=1), dict(parent=0), dict(point=1), dict(selection=[]),
                       dict(view=[]), dict(view='Front'), dict(input='repeat'),
                       dict(selection='all', parent=True)):
            with self.subTest(change=change), self.assertRaises(ValueError):
                validate(dict(q['operations'][0], **change))
        rhino = mock.Mock()
        rhino.RhinoDoc.ActiveDoc.Objects = [object()]
        rhino.Commands.Command.InCommand.return_value = False
        with self.assertRaisesRegex(ValueError, 'empty owned document'):
            run(q['operations'][0], {'Rhino': rhino, 'System': mock.Mock()})
        rhino.Commands.Command.InCommand.return_value = True
        with self.assertRaisesRegex(ValueError, 'idle execution'):
            run(q['operations'][0], {'Rhino': rhino, 'System': mock.Mock()})

    def test_private_launch_guards_before_any_process(self):
        for scheme, iterations, display, marker in ((None, 1, ':301', ':301'),
                ('VibocerosOracleScaleNU', True, ':301', ':301'),
                ('VibocerosOracleScaleNU', 2, ':301', ':301'),
                ('VibocerosOracleScaleNU', 1, ':1', None),
                ('VibocerosOracleScaleNU', 1, ':301', ':302')):
            q = request()
            q['iterations'] = iterations
            env = {'DISPLAY': display}
            if marker:
                env['VIBOCEROS_ORACLE_HEADLESS'] = marker
            with mock.patch.dict(os.environ, env, clear=True), mock.patch('tools.rhino_oracle.client._run_logged') as launch:
                with self.assertRaises((OracleError, OracleProtocolError)):
                    OracleClient(settings_scheme=scheme).run_rhino(q, 1)
                launch.assert_not_called()

    def test_native_signed_zero_frames_defaults_and_distance_reference(self):
        rows = read('observations')['results']
        self.assertEqual(len(rows), 26)
        self.assertEqual([o['id'] for o in request()['operations']], [r['id'] for r in rows])
        for row in rows:
            v = row['value']
            self.assertTrue(v['success'])
            self.assertEqual(v['after'], v['after_script'])
            self.assertEqual(v['redo'], v['after_script'])
            self.assertNotIn('_Cancel', v['macro'])
        expected = {0: [3., 5., 3.5], 1: [4., 4., 3.5], 3: [3., 1., 3.5],
                    4: [0., 3., 4.], 6: [4., 9., 2.], 7: [3., 5., 3.5],
                    13: [6., 3., 4.], 23: [2., 9., 4.], 24: [2., 3., 12.],
                    25: [12., 3., 4.]}
        for i, p in expected.items():
            self.assertEqual(rows[i]['value']['after'][0]['point'], p)
        for i, p in ((5, [6., 3., 4.]), (10, [6., 3., 4.]), (11, [2., 9., 4.]),
                     (12, [2., 3., 12.]), (20, [6., 3., 4.]), (22, [6., 3., 4.])):
            self.assertNotEqual(rows[i]['value']['after'][0]['point'], p)
        # The second prompt's number constrains distance and still awaits a
        # point: off-axis target [3,4,0] with distance 6 yields factor 6.
        self.assertIn('reference point <1.000>', rows[25]['value']['history'])
        self.assertNotEqual(rows[5]['value']['after'][0]['point'], rows[20]['value']['after'][0]['point'])

    def test_native_grip_copy_parent_order_history_and_collapsed_face_records(self):
        rows = read('observations')['results']
        v = rows[8]['value']
        self.assertEqual(v['after'][:2], v['before'])
        self.assertEqual([o['role'] for o in v['after']], ['source', 'point', 'output', 'output'])
        self.assertTrue(v['after'][-1]['grips_on'])
        self.assertFalse(v['after'][-1]['selected'])
        self.assertEqual([g['index'] for g in v['after'][-1]['grips'] if g['selected']], [0, 2])
        for i in (8, 15, 16, 17):
            self.assertEqual(rows[i]['value']['undo'], rows[i]['value']['before'])
        for i in (9, 21):
            v = rows[i]['value']
            self.assertEqual(v['after'][0]['mesh']['faces'], v['before'][0]['mesh']['faces'])
            self.assertEqual(v['undo'], v['before'])
        all_picks = rows[18]['value']
        self.assertTrue(all(not g['selected'] for g in all_picks['before'][0]['grips']))
        self.assertTrue(all(g['selected'] for g in all_picks['undo'][0]['grips']))

    def test_provenance(self):
        p = json.loads((ROOT / 'docs/scale-nu-provenance.json').read_text())
        self.assertTrue(p['private_xvfb'])
        self.assertFalse(p['full_native_parity'])
        self.assertEqual(p['native_recipes'], 26)
        self.assertEqual(p['positive_workflows'], 20)
        self.assertEqual(p['application_replays'], 40)
        self.assertEqual(p['known_incompatible_ids'], ['scale-nu-'+str(i) for i in (5, 10, 11, 12, 20, 22)])
        for path, expected in p['sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT / path).read_bytes()).hexdigest(), expected, path)
