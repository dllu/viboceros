"""Bounded grip recipes, retained native behavior, and private launch guards."""
import hashlib
import json
import os
from pathlib import Path
from unittest import TestCase, mock

from .grip_transform_probe import request, run, validate
from .client import OracleClient, OracleError, OracleProtocolError

ROOT = Path(__file__).resolve().parents[2]


def read(folder):
    return json.loads((ROOT / 'tools/rhino_oracle' / folder / 'grip_transform.json').read_text())


class GripTransformTests(TestCase):
    def test_closed_schema_and_owned_empty_document(self):
        q = request()
        self.assertEqual(q, read('fixtures'))
        for change in (dict(extra=True), dict(id='x\n_Delete'), dict(id='λ'),
                       dict(source='anything'), dict(source=[]), dict(command=[]),
                       dict(command='move _Exit'), dict(selected=[True]), dict(selected=[-1]),
                       dict(selected=[4]), dict(selected=[0, 0]), dict(selected=[]),
                       dict(parent=1), dict(point=0), dict(off=1), dict(inputs='all _Exit'),
                       dict(inputs='all')):
            with self.subTest(change=change), self.assertRaises(ValueError):
                validate(dict(q['operations'][0], **change))
        all_recipe = dict(q['operations'][26], parent=True)
        with self.assertRaises(ValueError):
            validate(all_recipe)
        rhino = mock.Mock()
        rhino.RhinoDoc.ActiveDoc.Objects = [object()]
        with self.assertRaisesRegex(ValueError, 'empty owned document'):
            run(q['operations'][0], {'Rhino': rhino, 'System': mock.Mock()})

    def test_private_scheme_display_and_iteration_guards_precede_launch(self):
        for scheme, count, display, headless in ((None, 1, ':301', ':301'),
                ('VibocerosOracleGrips', True, ':301', ':301'),
                ('VibocerosOracleGrips', 2, ':301', ':301'),
                ('VibocerosOracleGrips', 1, ':0', None),
                ('VibocerosOracleGrips', 1, ':301', ':302')):
            q = request()
            q['iterations'] = count
            env = {'DISPLAY': display}
            if headless:
                env['VIBOCEROS_ORACLE_HEADLESS'] = headless
            with mock.patch.dict(os.environ, env, clear=True), mock.patch('tools.rhino_oracle.client._run_logged') as launch:
                with self.assertRaises((OracleError, OracleProtocolError)):
                    OracleClient(settings_scheme=scheme).run_rhino(q, 1)
                launch.assert_not_called()

    def test_native_partial_edits_aliases_and_duplicate_mesh_vertices(self):
        q, r = read('fixtures'), read('observations')
        self.assertEqual(r['engine_version'], '8.32.26160.13001')
        self.assertEqual(len(r['results']), 32)
        self.assertEqual([o['id'] for o in q['operations']], [o['id'] for o in r['results']])
        for op, row in zip(q['operations'], r['results']):
            with self.subTest(id=op['id']):
                v = row['value']
                self.assertTrue(v['success'])
                self.assertEqual(v['after'], v['after_script'])
                self.assertNotIn('_Cancel', v['script_macro'])
        for row in r['results'][:11]:
            before, after = row['value']['before'][0], row['value']['after'][0]
            self.assertEqual([g['index'] for g in after['grips'] if g['selected']], [0])
            for old, new in zip(before['grips'], after['grips']):
                delta = [1., 2., 3.] if old['index'] == 0 else [0., 0., 0.]
                self.assertEqual(new['point'], [a+b for a, b in zip(old['point'], delta)])
            key = 'surface' if 'surface' in before else 'curve' if 'curve' in before else None
            if key:
                for attr in ('degree', 'domain', 'domain_u', 'domain_v', 'knots', 'knots_u', 'knots_v', 'control_count'):
                    if attr in before[key]:
                        self.assertEqual(before[key][attr], after[key][attr])
                self.assertEqual([p['weight'] for p in before[key]['control_points']],
                                 [p['weight'] for p in after[key]['control_points']])
        closed = r['results'][2]['value']['after'][0]['curve']['control_points']
        periodic = r['results'][3]['value']['after'][0]['curve']['control_points']
        self.assertEqual(closed[0], closed[-1])
        self.assertEqual(periodic[0], periodic[-2])
        mesh = r['results'][10]['value']
        self.assertEqual(mesh['before'][0]['mesh']['vertices'][0], mesh['before'][0]['mesh']['vertices'][4])
        self.assertNotEqual(mesh['after'][0]['mesh']['vertices'][0], mesh['after'][0]['mesh']['vertices'][4])
        # Retain the incompatible native collapsed triangle, including its indices.
        collapsed = r['results'][13]['value']['after'][0]['mesh']
        self.assertEqual(collapsed['vertices'][2], collapsed['vertices'][4])
        self.assertEqual(collapsed['faces'][-1], [4, 2, 3])

    def test_native_copy_parent_precedence_and_display_history_exchange(self):
        r = read('observations')['results']
        for i in (14, 15, 16, 28):
            v = r[i]['value']
            self.assertEqual(v['before'][0], v['after'][0])
            copy = v['after'][1]
            self.assertFalse(copy['selected'])
            self.assertTrue(copy['grips_on'])
            self.assertEqual([g['index'] for g in copy['grips'] if g['selected']], [0, 2])
            self.assertEqual(v['undo'], v['before'])
            self.assertEqual(v['redo'], v['after'])
        for i in (17, 18, 19):
            v = r[i]['value']
            for old, new in zip(v['before'][0]['grips'], v['after'][0]['grips']):
                delta = [1., 2., 3.] if old['index'] in (0, 2) else [0., 0., 0.]
                self.assertEqual(new['point'], [a+b for a, b in zip(old['point'], delta)])
        for i in (23, 25):
            v = r[i]['value']
            self.assertTrue(v['undo'][0]['grips_on'])
            self.assertFalse(v['redo'][0]['grips_on'])
        copy_off = r[24]['value']
        self.assertTrue(all(not o['grips_on'] for o in copy_off['undo'] + copy_off['redo']))
        mixed = r[29]['value']
        self.assertTrue(mixed['undo'][1]['selected'])
        self.assertTrue(mixed['redo'][1]['selected'])
        self.assertEqual([o['role'] for o in mixed['after']], ['source', 'point', 'output', 'output'])
        for i in (21, 22):
            self.assertEqual(r[i]['value']['undo'], [])
            self.assertEqual(r[i]['value']['redo'], r[i]['value']['before'])

    def test_native_endpoint_closing_prunes_vanished_grip_and_restores_it_on_undo(self):
        rows = read('observations')['results']
        move, copy = rows[30]['value'], rows[31]['value']
        self.assertEqual(len(move['before'][0]['grips']), 4)
        for state in (move['after'], move['redo'], copy['after'][1:], copy['redo'][1:]):
            self.assertEqual(len(state[0]['grips']), 3)
            self.assertTrue(all(not g['selected'] for g in state[0]['grips']))
            controls = state[0]['curve']['control_points']
            self.assertEqual(controls[0], controls[-1])
        self.assertEqual(move['undo'], move['before'])
        self.assertEqual(copy['after'][0], copy['before'][0])
        self.assertEqual(copy['undo'], copy['before'])

    def test_provenance_hashes(self):
        record = json.loads((ROOT / 'docs/grip-transform-provenance.json').read_text())
        self.assertTrue(record['private_xvfb'])
        self.assertFalse(record['full_native_parity'])
        self.assertEqual(record['native_recipes'], 32)
        self.assertEqual(record['positive_workflows'], 31)
        self.assertEqual(record['known_incompatible_ids'], ['grip-transform-13'])
        for path, expected in record['sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT / path).read_bytes()).hexdigest(), expected, path)
