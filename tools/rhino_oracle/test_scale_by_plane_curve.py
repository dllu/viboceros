"""Independent public CPlane witnesses for native curve representations."""
import copy
import hashlib
import json
import math
import os
from pathlib import Path
from unittest import TestCase, mock

from .client import OracleClient, OracleError, OracleProtocolError
from .scale_by_plane_curve_probe import TARGETS, request, run, validate_request
from .test_scale_by_plane_object import mapped

ROOT = Path(__file__).resolve().parents[2]


def normalized(delta):
    length = math.sqrt(sum(x*x for x in delta))
    return [x/length for x in delta]


class ScaleByPlaneCurveTests(TestCase):
    def captures(self):
        q = json.loads((ROOT/'tools/rhino_oracle/fixtures/scale_by_plane_curve.json').read_text())
        r = json.loads((ROOT/'tools/rhino_oracle/observations/scale_by_plane_curve.json').read_text())
        self.assertEqual(q, request()); validate_request(q)
        self.assertEqual(r['engine'], 'rhino')
        self.assertEqual(len(r['results']), 64)
        self.assertEqual([op['id'] for op in q['operations']], [row['id'] for row in r['results']])
        return [(op, row['value']) for op, row in zip(q['operations'], r['results'])]

    def near(self, a, b, label):
        self.assertEqual(len(a), len(b), label)
        for x, y in zip(a, b):
            self.assertAlmostEqual(x, y, delta=1e-9, msg=label)

    def test_independent_planes_predict_outputs_and_native_history(self):
        for op, v in self.captures():
            label = op['id']
            self.assertEqual([e['result'] for e in v['cplane_events'] if e['name']=='CPlane'], ['Success'], label)
            events = [e for e in v['events'] if e['name']=='ScaleByPlane']
            self.assertEqual(len(events), 1, label)
            self.assertEqual(events[0]['result'], 'Success', label)
            self.assertEqual(events[0]['objects'], v['after'], label)
            self.assertEqual(v['after'], v['after_script'], label)
            self.assertEqual(v['after'], v['redo'], label)
            self.assertEqual(len(v['after']), 4, label)
            before = copy.deepcopy(v['before'])
            for source in before: source['selected'] = False
            self.assertEqual(before, v['undo'], label)
            self.assertEqual([o['point'] for o in v['before']], [[2.,3.,4.],[2.,2.,3.],[1.,3.,3.],[1.,2.,4.]], label)
            for source, output in zip(v['before'], v['after']):
                self.near(output['point'], mapped(source['point'], v), label)
                self.assertEqual(output['name'], source['name'], label)
                self.assertEqual(output['role'], 'source', label)
                self.assertFalse(output['selected'], label)

    def test_analytic_and_nurbs_arc_planes_remain_distinct(self):
        cases = {(op['target'],op['plane'],op['reverse']): v for op,v in self.captures()}
        analytic = circular = ellipses = 0
        for op, v in self.captures():
            target, label = v['target'], op['id']
            if op['target'].startswith('single_'): target = target['segments'][0]
            if 'arc' in target:
                analytic += 1
                for key in ('origin','x_axis','y_axis'):
                    self.near(v['cplane'][key], target['arc']['plane'][key], label)
            elif op['target'] in ('nurbs_circle','nurbs_arc','nurbs_offset_arc'):
                circular += 1
                controls = target['curve']['control_points']
                center = v['target_frame']['origin']
                self.near(v['cplane']['origin'], center, label)
                self.near(v['cplane']['x_axis'], normalized([x-y for x,y in zip(controls[0]['point'],center)]), label)
                self.near(v['cplane']['y_axis'], normalized([x-y for x,y in zip(controls[1]['point'],controls[0]['point'])]), label)
            elif op['target'] in ('ellipse','nurbs_ellipse','single_ellipse'):
                ellipses += 1
                controls = target['curve']['control_points']
                self.near(v['cplane']['origin'], controls[0]['point'], label)
                self.near(v['cplane']['x_axis'], normalized([x-y for x,y in zip(controls[1]['point'],controls[0]['point'])]), label)
        self.assertEqual((analytic,circular,ellipses), (24,12,12))
        for plane in ('world','tilted'):
            for reverse in (False,True):
                a, n = cases['offset_arc',plane,reverse], cases['nurbs_offset_arc',plane,reverse]
                self.assertGreater(max(abs(x-y) for x,y in zip(a['cplane']['x_axis'],n['cplane']['x_axis'])), .1)
                self.assertGreater(max(abs(x-y) for p,q in zip(a['after'],n['after']) for x,y in zip(p['point'],q['point'])), .01)

    def test_closed_recipes_and_owned_idle_guards(self):
        q = request(); op = q['operations'][0]; validate_request(q)
        self.assertEqual({o['target'] for o in q['operations']},set(TARGETS))
        for change in (dict(op='Delete'),dict(script='_Delete'),dict(id='x\n_Delete'),dict(id=[]),dict(target=[]),dict(target='unknown'),dict(plane=[]),dict(plane='unknown'),dict(reverse=0)):
            with self.subTest(change=change), self.assertRaises(ValueError):
                validate_request(dict(protocol_version=1,operations=[dict(op,**change)]))
        for bad in (None,[],dict(protocol_version=True,operations=[op]),dict(protocol_version=1,iterations=True,operations=[op]),dict(protocol_version=1,iterations=2,operations=[op]),dict(protocol_version=1,operations=[]),dict(protocol_version=1,operations=[op,op]),dict(protocol_version=1,operations=[dict(op,id=str(i)) for i in range(65)])):
            with self.assertRaises(ValueError): validate_request(bad)
        rhino = mock.Mock(); rhino.Commands.Command.InCommand.return_value = True
        with self.assertRaisesRegex(ValueError,'idle execution'): run(op,dict(Rhino=rhino,System=mock.Mock()))
        rhino.Commands.Command.InCommand.return_value = False; rhino.RhinoDoc.ActiveDoc.Objects = [object()]
        with self.assertRaisesRegex(ValueError,'empty owned document'): run(op,dict(Rhino=rhino,System=mock.Mock()))

    def test_private_guard_precedes_launch(self):
        for scheme,display,marker in ((None,':301',':301'),('VibocerosOracleCurve',':1',None),('VibocerosOracleCurve',':301',':302')):
            env = dict(DISPLAY=display)
            if marker: env['VIBOCEROS_ORACLE_HEADLESS'] = marker
            with mock.patch.dict(os.environ,env,clear=True),mock.patch('tools.rhino_oracle.client._run_logged') as launch:
                with self.assertRaises((OracleError,OracleProtocolError)):
                    OracleClient(settings_scheme=scheme).run_rhino(request(),1)
                launch.assert_not_called()

    def test_provenance(self):
        p = json.loads((ROOT/'docs/scale-by-plane-curve-provenance.json').read_text())
        self.assertTrue(p['private_xvfb']); self.assertFalse(p['full_native_parity'])
        self.assertEqual((p['native_recipes'],p['application_replays'],p['point_cplane_replays'],p['native_3dm_round_trip_objects']), (64,128,20,24))
        for path,sha in p['sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(),sha,path)
