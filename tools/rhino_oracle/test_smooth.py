"""Closed Smooth recipes, native snapshots and private-display launch guards."""
import hashlib
import json
import os
from pathlib import Path
from unittest import TestCase, mock

from .client import OracleClient, OracleError, OracleProtocolError
from .smooth_probe import request, run, validate_request
from .smooth_frames_probe import validate_request as validate_frames

ROOT = Path(__file__).resolve().parents[2]


def read(folder, name):
    return json.loads((ROOT / 'tools/rhino_oracle' / folder / (name+'.json')).read_text())


class SmoothTests(TestCase):
    def test_closed_recipe_schema_rejects_script_and_unbounded_requests(self):
        q = request()
        self.assertEqual(q, read('fixtures', 'smooth_fixed'))
        for change in (dict(extra=True), dict(id='x\n_Delete'), dict(id='λ'),
                       dict(id=[]), dict(source='anything'), dict(source=[]),
                       dict(selection='all _Exit'), dict(selection=[]),
                       dict(mode='defaults _Exit'), dict(mode=[])):
            with self.subTest(change=change), self.assertRaises(ValueError):
                validate_request(dict(q, operations=[dict(q['operations'][0], **change)]))
        for change in (dict(protocol_version=True), dict(iterations=True),
                       dict(iterations=2), dict(operations=[]),
                       dict(operations=q['operations']+[q['operations'][0]]),
                       dict(operations=[q['operations'][0], q['operations'][0]])):
            with self.subTest(change=change), self.assertRaises(ValueError):
                validate_request(dict(q, **change))
        rhino = mock.Mock()
        rhino.Commands.Command.InCommand.return_value = 0
        rhino.RhinoDoc.ActiveDoc.Objects = [object()]
        with self.assertRaisesRegex(ValueError, 'empty owned document'):
            run(q['operations'][0], dict(Rhino=rhino, System=mock.Mock()))

    def test_sdk_witness_schema(self):
        q = dict(protocol_version=1, iterations=1, operations=[dict(
            op='smooth_frames', id='witness', source='curve', mode='curve')])
        validate_frames(q)
        for change in (dict(extra=True), dict(id='bad\n_SelAll'),
                       dict(mode='script'), dict(source='script'), dict(source=[])):
            with self.subTest(change=change), self.assertRaises(ValueError):
                validate_frames(dict(q, operations=[dict(q['operations'][0], **change)]))

    def test_private_settings_display_and_iterations_precede_launch(self):
        for kind in ('smooth_command', 'smooth_frames'):
            for scheme, count, display, headless in ((None, 1, ':301', ':301'),
                    ('VibocerosOracleSmooth', True, ':301', ':301'),
                    ('VibocerosOracleSmooth', 2, ':301', ':301'),
                    ('VibocerosOracleSmooth', 1, ':1', None),
                    ('VibocerosOracleSmooth', 1, ':301', ':302')):
                op = dict(op=kind, id='guard', source='curve', mode='curve' if kind=='smooth_frames' else 'free')
                if kind=='smooth_command': op['selection'] = 'objects'
                q = dict(protocol_version=1, iterations=count, operations=[op])
                env = dict(DISPLAY=display)
                if headless: env['VIBOCEROS_ORACLE_HEADLESS'] = headless
                with self.subTest(kind=kind, scheme=scheme, count=count, display=display), \
                        mock.patch.dict(os.environ, env, clear=True), \
                        mock.patch('tools.rhino_oracle.client._run_logged') as launch:
                    with self.assertRaises((OracleError, OracleProtocolError, ValueError)):
                        OracleClient(settings_scheme=scheme).run_rhino(q, 1)
                    launch.assert_not_called()

    def test_native_command_end_history_promotions_and_grip_precedence(self):
        total = 0
        for name, count in (('smooth_fixed', 96), ('smooth_selected', 32)):
            q, r = read('fixtures', name), read('observations', name)
            self.assertEqual(r['engine_version'], '8.32.26160.13001')
            self.assertEqual(len(r['results']), count)
            self.assertEqual([o['id'] for o in q['operations']], [o['id'] for o in r['results']])
            for op, row in zip(q['operations'], r['results']):
                total += 1
                with self.subTest(id=op['id']):
                    v = row['value']
                    self.assertTrue(v['success'])
                    self.assertTrue(v['command_registered'])
                    ends = [e for e in v['events'] if e['name']=='Smooth']
                    self.assertEqual([e['result'] for e in ends], ['Success'])
                    self.assertEqual(ends[0]['objects'], v['after'])
                    self.assertEqual(v['undo'], v['before'])
                    self.assertEqual(v['redo'], v['after_script'])
                    self.assertEqual(len(v['before']), 1)
                    self.assertEqual(len(v['after']), 1)
                    before, after = v['before'][0], v['after'][0]
                    self.assertEqual(after['name'], 'smooth source')
                    self.assertEqual(after['grips_on'], before['grips_on'])
                    self.assertEqual([g['index'] for g in before['grips'] if g['selected']],
                                     [g['index'] for g in after['grips'] if g['selected']])
                    key = 'curve' if 'curve' in before else 'surface' if 'surface' in before else 'mesh'
                    if key=='mesh':
                        self.assertEqual(before[key]['faces'], after[key]['faces'])
                    else:
                        for k in before[key]:
                            if k!='control_points': self.assertEqual(before[key][k], after[key][k])
                        self.assertEqual([p['weight'] for p in before[key]['control_points']],
                                         [p['weight'] for p in after[key]['control_points']])
                    if op['source'] in ('line','polyline','circle','arc'):
                        self.assertEqual(after['kind'], 'NurbsCurve')
                    if op['mode'] in ('zero','none'):
                        self.assertEqual(before[key], after[key])
            groups = {}
            for op, row in zip(q['operations'], r['results']):
                groups.setdefault((op['source'],op['mode']), {})[op['selection']] = row['value']
            for selections in groups.values():
                if 'grips' in selections and 'parent' in selections:
                    a, b = selections['grips']['after'][0], selections['parent']['after'][0]
                    for key in ('curve','surface','mesh'):
                        if key in a: self.assertEqual(a[key], b[key])
        self.assertEqual(total, 128)

    def test_object_x_sdk_predictions_and_alternative_counterexamples(self):
        q, r = read('fixtures','smooth_object'), read('observations','smooth_object')
        self.assertEqual(len(r['results']),80)
        actual = {}
        for op,row in zip(q['operations'],r['results']):
            self.assertEqual(op['id'],row['id'])
            self.assertTrue(row['value']['success'])
            if op['mode']=='object_x': actual[op['source']] = row['value']['after'][0]
        f, witnesses = read('fixtures','smooth_frames'), read('observations','smooth_frames')
        self.assertEqual(len(witnesses['results']),18)
        accepted, alternatives = 0,0
        for op,row in zip(f['operations'],witnesses['results']):
            self.assertEqual(op['id'],row['id'])
            key = 'curve' if op['mode']=='curve' else 'surface' if op['mode'].startswith('surface') else 'mesh'
            field = 'vertices' if key=='mesh' else 'control_points'
            a,b = actual[op['source']][key][field],row['value']['predicted'][field]
            if key!='mesh': a,b = [p['point'] for p in a],[p['point'] for p in b]
            self.assertEqual(len(a),len(b))
            error = max(abs(x-y) for p,q in zip(a,b) for x,y in zip(p,q))
            with self.subTest(source=op['source'], mode=op['mode']):
                if op['mode'] in ('curve','surface_normal_u','mesh_static'):
                    accepted += 1
                    self.assertLessEqual(error,2e-12)
                else:
                    alternatives += 1
                    self.assertGreater(error,1e-4)
        self.assertEqual((accepted,alternatives),(10,8))
        periodic = witnesses['results'][2]['value']['rows']
        self.assertFalse(periodic[0]['available'])
        self.assertFalse(periodic[-1]['available'])
        self.assertNotEqual(periodic[0]['tangent'],[0.,0.,0.])
        controls = actual['periodic']['curve']['control_points']
        self.assertNotEqual(controls[0]['point'],controls[-2]['point'])

    def test_object_all_axes_match_world_in_captured_families(self):
        q,r = read('fixtures','smooth_fixed'),read('observations','smooth_fixed')
        world = {o['source']:v['value']['after'][0] for o,v in zip(q['operations'],r['results'])
                 if o['mode']=='free' and o['selection']=='objects'}
        q,r = read('fixtures','smooth_object'),read('observations','smooth_object')
        compared = 0
        for op,row in zip(q['operations'],r['results']):
            if op['mode']!='object': continue
            compared += 1
            a,b = row['value']['after'][0],world[op['source']]
            key = 'curve' if 'curve' in a else 'surface' if 'surface' in a else 'mesh'
            field = 'vertices' if key=='mesh' else 'control_points'
            x,y = a[key][field],b[key][field]
            if key!='mesh': x,y = [p['point'] for p in x],[p['point'] for p in y]
            self.assertEqual(len(x),len(y))
            self.assertLessEqual(max(abs(c-d) for p,q in zip(x,y) for c,d in zip(p,q)),2e-12)
        self.assertEqual(compared,16)

    def test_provenance_hashes_and_scope(self):
        record = json.loads((ROOT / 'docs/smooth-provenance.json').read_text())
        self.assertTrue(record['private_xvfb'])
        self.assertFalse(record['full_native_parity'])
        self.assertFalse(record['command_registered_in_viboceros'])
        self.assertEqual(record['native_recipes'], 208)
        self.assertEqual(record['fixed_coordinate_recipes'], 128)
        self.assertEqual(record['object_coordinate_recipes'],80)
        self.assertEqual(record['sdk_witnesses'],18)
        self.assertEqual(record['kernel_replays'], 128)
        self.assertEqual(record['application_replays'], 0)
        for path, expected in record['sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT / path).read_bytes()).hexdigest(), expected, path)
