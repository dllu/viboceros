"""Real ScalePositions cursor input, raw history and independent placement equations."""
import copy
import hashlib
import json
import math
import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import Mock, patch

from .client import OracleClient, OracleError, OracleProtocolError
from .move_normal_probe import validate_camera
from .scale_positions_cursor_input import ScalePositionsCursorPicker
from .scale_positions_cursor_probe import origin_request, recipe, request, run, validate_request
from .translation_input import validate_frame

ROOT = Path(__file__).parent


def dot(a, b):
    return sum(x*y for x, y in zip(a, b))


def destination(op, value):
    """Independent viewing-ray intersection; no fitted native output coordinates."""
    spec = recipe(op)
    origin = spec['origin']
    if op['finish']=='Typed': return [6.,3.,2.]
    a, b = value['pending']['frame']['ray']
    ray = [y-x for x,y in zip(a,b)]
    if op['input']=='Reference' and op['mode']=='1d':
        axis = [x-y for x,y in zip(spec['reference'],origin)]
        relative = [x-y for x,y in zip(a,origin)]
        length2 = dot(ray,ray)
        t = ((dot(axis,relative)*length2-dot(axis,ray)*dot(ray,relative)) /
             (dot(axis,axis)*length2-dot(axis,ray)**2))
        return [x+t*y for x,y in zip(origin,axis)]
    t = (origin[2]-a[2])/ray[2]
    return [x+t*y for x,y in zip(a,ray)]


def placement(op, value):
    spec = recipe(op)
    origin = spec['origin']
    center = value['bounds']['center']
    if op['finish']=='ClickCancel' or (op['input']!='Reference' and max(abs(x) for x in origin)<=2.**-23):
        return None, spec['factor'] if op['input']=='Default' else 2.
    target = destination(op,value)
    direction = [x-y for x,y in zip(spec['reference'] or target,origin)]
    factor = spec['factor']
    if op['input']=='Reference':
        relative = [x-y for x,y in zip(target,origin)]
        factor = abs(dot(relative,direction)/dot(direction,direction)) if op['mode']=='1d' else math.sqrt(dot(relative,relative)/dot(direction,direction))
    relative = [x-y for x,y in zip(center,origin)]
    if op['mode']=='1d':
        delta = [x*(factor-1.)*dot(relative,direction)/dot(direction,direction) for x in direction]
    else:
        delta = [(factor-1.)*x if op['mode']=='3d' or i<2 else 0. for i,x in enumerate(relative)]
    return delta, factor


class ScalePositionsCursorTests(unittest.TestCase):
    def test_closed_bounded_recipes(self):
        for factory, name, count in ((request, 'cursor', 64), (origin_request, 'origin', 64)):
            q = factory()
            self.assertEqual(q, json.loads((ROOT/('fixtures/scale_positions_'+name+'.json')).read_text()))
            validate_request(q)
            self.assertEqual(len(q['operations']), count)
        case = request()['operations'][0]
        for changes in (dict(id='x\n_Delete'), dict(id='λ'), dict(id=[]), dict(op='Delete'),
                dict(script='_Delete'), dict(input=[]), dict(input='Scripted'), dict(mode=[]),
                dict(mode='2d'), dict(view=[]), dict(view='Delete'), dict(aim=[]), dict(aim='custom'),
                dict(view='Top', aim='Vertical'), dict(finish=[]), dict(finish='Return'),
                dict(finish='ClickCancel'), dict(finish='ClickCancel', view='Front', copy=True),
                dict(input='Reference', finish='ClickCancel', view='Front'), dict(copy=1),
                dict(source=[]), dict(source='mesh'), dict(origin=[]), dict(origin='Custom'),
                dict(factor=True), dict(factor=float('nan')), dict(factor=4),
                dict(input='Reference', factor=3)):
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                validate_request(dict(protocol_version=1, operations=[dict(case, **changes)]))
        for q in (None, [], dict(protocol_version=True, operations=[case]),
                dict(protocol_version=1, iterations=True, operations=[case]),
                dict(protocol_version=1, iterations=2, operations=[case]),
                dict(protocol_version=1, operations=[]), dict(protocol_version=1, operations=[case, case]),
                dict(protocol_version=1, operations=[dict(case,id=str(i)) for i in range(65)])):
            with self.subTest(request=q), self.assertRaises(ValueError): validate_request(q)

    def test_macro_origin_tokens_preserve_the_requested_double(self):
        for op in origin_request()['operations']:
            spec = recipe(op)
            token = spec['macro'].split()[2]
            self.assertTrue(token.startswith('w'))
            self.assertEqual([float(x) for x in token[1:].split(',')], spec['origin'], op['id'])

    def test_idle_owned_document_and_private_launch_guards(self):
        q = request()
        rhino = Mock()
        rhino.Commands.Command.InCommand.return_value = False
        rhino.RhinoDoc.ActiveDoc.Objects = [object()]
        with self.assertRaisesRegex(ValueError, 'empty owned document'):
            run(q['operations'][0], {'Rhino': rhino, 'System': Mock()})
        rhino.Commands.Command.InCommand.return_value = True
        with self.assertRaisesRegex(ValueError, 'idle execution'):
            run(q['operations'][0], {'Rhino': rhino, 'System': Mock()})
        for scheme, iterations, display, marker in ((None,1,':301',':301'),
                ('VibocerosOracleScalePositionsCursor',True,':301',':301'),
                ('VibocerosOracleScalePositionsCursor',2,':301',':301'),
                ('VibocerosOracleScalePositionsCursor',1,':1',None),
                ('VibocerosOracleScalePositionsCursor',1,':301',':302')):
            env = {'DISPLAY': display}
            if marker: env['VIBOCEROS_ORACLE_HEADLESS'] = marker
            with patch.dict(os.environ, env, clear=True), patch('tools.rhino_oracle.client._run_logged') as launch:
                with self.assertRaises((OracleError, OracleProtocolError, ValueError)):
                    OracleClient(settings_scheme=scheme).run_rhino(dict(q, iterations=iterations), 1)
                launch.assert_not_called()

    def test_picker_requires_native_motion_and_owned_coordinates(self):
        op = request()['operations'][0]
        marker = '@scale-positions-cursor:'+op['id']
        picker = ScalePositionsCursorPicker(dict(protocol_version=1, operations=[op]))
        with tempfile.TemporaryDirectory() as folder, patch('tools.rhino_oracle.scale_positions_cursor_input.subprocess.run') as send, patch('tools.rhino_oracle.scale_positions_cursor_input.time.monotonic', return_value=0.):
            picker.job = Path(folder)
            self.assertFalse(picker.send_input(marker, '100', '120', 'owned'))
            self.assertEqual(send.call_args.args[0], ['xdotool','windowactivate','--sync','owned','mousemove','101','120','mousemove','100','120'])
            self.assertFalse(picker.send_input(marker, '100', '120', 'owned'))
            self.assertEqual(send.call_count, 1)
            for args in ((marker,'100','120','foreign'), (marker,'101','120','owned'), ('@unexpected','100','120','owned')):
                with self.assertRaises(OracleProtocolError): picker.send_input(*args)
            ready = picker.job/('scale-positions-cursor-ready-'+op['id']+'.json')
            ready.write_text(json.dumps('foreign-marker'))
            with self.assertRaises(OracleProtocolError): picker.send_input(marker, '100', '120', 'owned')
            ready.write_text(json.dumps(marker))
            with patch('tools.rhino_oracle.scale_positions_cursor_input.time.monotonic', return_value=.3):
                self.assertTrue(picker.send_input(marker, '100', '120', 'owned'))
            self.assertEqual(send.call_args.args[0], ['xdotool','windowactivate','--sync','owned','click','1'])

    def test_typed_direction_and_rejected_click_have_native_acknowledgements(self):
        for index in (1,4):
            op = request()['operations'][index]
            marker = '@scale-positions-cursor:'+op['id']
            picker = ScalePositionsCursorPicker(dict(protocol_version=1, operations=[op]))
            picker.moved[marker] = ('owned','100','120',0.)
            with tempfile.TemporaryDirectory() as folder, patch('tools.rhino_oracle.scale_positions_cursor_input.subprocess.run') as send, patch('tools.rhino_oracle.scale_positions_cursor_input.time.monotonic', return_value=.3), patch('tools.rhino_oracle.scale_positions_cursor_input.time.sleep') as settle:
                picker.job = Path(folder)
                (picker.job/('scale-positions-cursor-ready-'+op['id']+'.json')).write_text(json.dumps(marker))
                if op['finish']=='Typed':
                    self.assertTrue(picker.send_input(marker,'100','120','owned'))
                    self.assertEqual(send.call_args_list[0].args[0], ['xdotool','windowactivate','--sync','owned','type','--clearmodifiers','--delay','30',recipe(op)['typed_target']])
                    self.assertEqual(send.call_args.args[0], ['xdotool','windowactivate','--sync','owned','key','--clearmodifiers','Return'])
                    settle.assert_called_once_with(.25)
                else:
                    self.assertFalse(picker.send_input(marker,'100','120','owned'))
                    self.assertEqual(send.call_args.args[0][-2:], ['click','1'])
                    self.assertFalse(picker.send_input(marker,'100','120','owned'))
                    self.assertEqual(send.call_count,1)
                    rejected = picker.job/('scale-positions-cursor-rejected-'+op['id']+'.json')
                    rejected.write_text(json.dumps('foreign-marker'))
                    with self.assertRaises(OracleProtocolError): picker.send_input(marker,'100','120','owned')
                    rejected.write_text(json.dumps(marker))
                    self.assertTrue(picker.send_input(marker,'100','120','owned'))
                    self.assertEqual(send.call_args.args[0], ['xdotool','windowactivate','--sync','owned','key','--clearmodifiers','Escape'])
                    settle.assert_not_called()

    def test_complete_response_order_and_raw_values_are_preserved(self):
        q = request()
        picker = ScalePositionsCursorPicker(q)
        response = dict(results=[dict(id=op['id'],value={'raw':True}) for op in q['operations']])
        original = copy.deepcopy(response)
        with self.assertRaises(OracleProtocolError): picker.record_diagnostics(response)
        picker.seen = set(picker.cases)
        picker.record_diagnostics(response)
        self.assertEqual(response, original)
        for rows in (None, [], list(reversed(response['results'])), [{}], [None]):
            with self.assertRaises(OracleProtocolError): picker.record_diagnostics(dict(results=rows))

    def test_native_cursor_geometry_and_history_follow_independent_equations(self):
        self.check_observations(request(),'cursor')

    def test_native_origin_default_and_boundary_equations(self):
        self.check_observations(origin_request(),'origin')

    def test_repeated_boundary_discrepancy_remains_raw(self):
        q = origin_request()
        q['operations'] = q['operations'][-8:]
        self.assertEqual(q,json.loads((ROOT/'fixtures/scale_positions_boundary_diagnostic.json').read_text()))
        repeat = json.loads((ROOT/'observations/scale_positions_boundary_diagnostic.json').read_text())
        primary = json.loads((ROOT/'observations/scale_positions_origin.json').read_text())
        self.assertEqual([op['id'] for op in q['operations']],[r['id'] for r in repeat['results']])
        for op,row in zip(q['operations'],repeat['results']):
            v = row['value']; self.assertEqual(v['recipe'],recipe(op))
            self.assertEqual(v['before'],v['pending']['objects'])
            self.assertEqual(v['after'],v['after_script'])
            self.assertEqual(v['after'],v['redo'])
        edited = repeat['results'][4]['value']
        unchanged = primary['results'][60]['value']
        self.assertEqual(edited['recipe'],unchanged['recipe'])
        self.assertEqual(edited['before'],unchanged['before'])
        self.assertEqual(edited['undo'],edited['before'])
        self.assertEqual(edited['preferences']['factor'],3.)
        self.assertEqual(unchanged['preferences']['factor'],2.)
        self.assertEqual(unchanged['before'],unchanged['after'])
        self.assertEqual(unchanged['undo'],[])
        delta,_ = placement(q['operations'][4],edited)
        for a,b,d in zip(edited['after'][0]['point'],edited['before'][0]['point'],delta):
            self.assertAlmostEqual(a,b+d,delta=1e-9)
        self.assertGreater(max(abs(a-b) for a,b in zip(edited['after'][0]['point'],unchanged['after'][0]['point'])),1e-6)

    def check_observations(self, q, name):
        observed = json.loads((ROOT/('observations/scale_positions_'+name+'.json')).read_text())
        self.assertEqual(observed['engine'], 'rhino')
        self.assertEqual([op['id'] for op in q['operations']], [row['id'] for row in observed['results']])
        for op,row in zip(q['operations'],observed['results']):
            v = row['value']; label = op['id']
            self.assertTrue(v['success'],label)
            self.assertEqual(v['recipe'],recipe(op),label)
            seed = '_ScalePositions _Copy=_No _Mode=_'+op['mode'].upper()+' w0,0,0 w1,0,0 w2,0,0'
            if op['input']=='Default' and op['factor']!=2.:
                seed = '_ScalePositions _Copy=_No _Mode=_'+op['mode'].upper()+' w0,0,0 w2,0,0 w'+str(2.*op['factor'])+',0,0'
            self.assertEqual(v['seed_macro'],seed,label)
            end = [event for event in v['events'] if event['name']=='ScalePositions']
            self.assertEqual(len(end),1,label)
            self.assertEqual(end[0]['objects'],v['after'],label)
            if op['finish']=='Typed': self.assertIn('w6,3,2',v['history'],label)
            self.assertEqual(v['after'],v['after_script'],label)
            self.assertEqual(v['before'],v['pending']['objects'],label)
            self.assertEqual(v['after'],v['redo'],label)
            validate_frame(v['pending']['frame'])
            validate_camera(v['pending']['camera'],v['pending']['frame'])
            self.assertEqual(v['pending']['phase'], 'second_reference' if op['input']=='Reference' else 'direction',label)
            if op['finish']=='ClickCancel':
                self.assertEqual(v['pending']['rejected_click']['objects'],v['before'],label)
                self.assertIn('Scale direction',v['pending']['rejected_click']['prompt'],label)
            delta, factor = placement(op,v)
            if op['origin']=='FloatAbove':
                # Retain the immediate-above boundary as a diagnostic. A
                # separate repeat made an in-place edit for the same request.
                # Do not rewrite either output or count it as an app match.
                self.assertIsNotNone(delta,label)
                self.assertGreater(max(abs(x) for x in delta),1e-6,label)
                self.assertEqual(v['before'],v['after'],label)
                self.assertEqual(v['undo'],[],label)
                self.assertEqual(v['preferences'],dict(mode='1D',copy=op['copy'],factor=2.),label)
                continue
            self.assertEqual(v['preferences']['mode'],op['mode'].upper(),label)
            self.assertEqual(v['preferences']['copy'],op['copy'],label)
            self.assertAlmostEqual(v['preferences']['factor'],factor,delta=5e-6,msg=label)
            if delta is None:
                self.assertEqual(v['before'],v['after'],label)
                self.assertEqual(v['undo'],[],label)
                continue
            self.assertEqual(v['before'],v['undo'],label)
            source = v['before'][0]
            expected = copy.deepcopy(source)
            points = [expected['point']] if op['source']=='point' else [c['point'] for c in expected['curve']['control_points']]
            for point in points:
                for i in range(3): point[i] += delta[i]
            index = 1 if op['copy'] else 0
            if op['copy']:
                expected['selected'] = False
                self.assertEqual(v['after'][0],source,label)
            self.assertEqual(len(v['after']),index+1,label)
            actual = v['after'][index]
            actual_points = [actual['point']] if op['source']=='point' else [c['point'] for c in actual['curve']['control_points']]
            for a,b in zip(actual_points,points):
                for x,y in zip(a,b): self.assertAlmostEqual(x,y,delta=1e-9,msg=label)
            # Shape, control weights, knots, degree and domain must all survive.
            canonical = copy.deepcopy(actual)
            if op['source']=='point': canonical['point'] = expected['point']
            else:
                for a,b in zip(canonical['curve']['control_points'],expected['curve']['control_points']): a['point'] = b['point']
            self.assertEqual(canonical,expected,label)

    def test_provenance_and_explicit_diagnostic_counts(self):
        provenance = json.loads((ROOT.parents[1]/'docs/scale-positions-cursor-provenance.json').read_text())
        self.assertTrue(provenance['private_xvfb'])
        self.assertFalse(provenance['full_native_parity'])
        self.assertEqual(provenance['native_recipes'],128)
        self.assertEqual(provenance['positive_application_replays'],126)
        self.assertEqual(provenance['repeat_boundary_recipes'],8)
        self.assertEqual(provenance['native_observations'],136)
        diagnostics = [op['id'] for op in origin_request()['operations'] if op['origin']=='FloatAbove']
        self.assertEqual(provenance['unresolved_application_recipes'],diagnostics)
        self.assertEqual(len(diagnostics),2)
        for scheme in provenance['settings_schemes']: self.assertTrue(scheme.startswith('VibocerosOracle'))
        for path,expected in provenance['sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT.parents[1]/path).read_bytes()).hexdigest(),expected,path)
        for name in ('cursor','origin','boundary_diagnostic'):
            response = json.loads((ROOT/('observations/scale_positions_'+name+'.json')).read_text())
            self.assertEqual(response['engine_version'],provenance['engine_version'])


if __name__ == '__main__': unittest.main()
