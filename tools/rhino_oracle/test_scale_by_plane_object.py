"""Independent CPlane witnesses for ScaleByPlane Object frames and history."""
import copy
import hashlib
import json
import os
from pathlib import Path
from unittest import TestCase, mock

from .client import OracleClient, OracleError, OracleProtocolError
from .scale_by_plane_object_probe import request, run, validate_request
from .scale_by_plane_object_input import ScaleByPlaneObjectPicker
from .test_scale_nu_options import dot

ROOT = Path(__file__).resolve().parents[2]


def mapped(point, value):
    origin = value['origin']
    delta = [x-y for x,y in zip(point,origin)]
    result = list(point)
    for key in ('x_axis','y_axis'):
        axis = value['cplane'][key]
        numerator = dot([x-y for x,y in zip(value['destination'],origin)],axis)
        denominator = dot([x-y for x,y in zip(value['reference'],origin)],axis)
        factor = numerator/denominator if min(abs(numerator),abs(denominator))>1e-6 else 1.
        result = [x+(factor-1.)*dot(delta,axis)*u for x,u in zip(result,axis)]
    return result


class ScaleByPlaneObjectTests(TestCase):
    def captures(self):
        q = json.loads((ROOT/'tools/rhino_oracle/fixtures/scale_by_plane_object.json').read_text())
        r = json.loads((ROOT/'tools/rhino_oracle/observations/scale_by_plane_object.json').read_text())
        self.assertEqual(q,request()); validate_request(q)
        self.assertEqual(len(q['operations']),64)
        self.assertEqual([op['id'] for op in q['operations']],[row['id'] for row in r['results']])
        return list(zip(q['operations'],r['results']))

    def test_separate_native_cplane_axes_predict_all_four_independent_points(self):
        accepted = rejected = copies = nonplanar = 0
        for op,row in self.captures():
            v,label = row['value'],op['id']
            accepted += v['object_accepted']; rejected += not v['object_accepted']; copies += op['copy']
            cplane = [e for e in v['cplane_events'] if e['name']=='CPlane']
            events = [e for e in v['events'] if e['name']=='ScaleByPlane']
            self.assertEqual(len(events),1,label); self.assertEqual(len(cplane),1,label)
            self.assertEqual(events[0]['objects'],v['after'],label)
            self.assertEqual(v['object_accepted'],op['target']!='mesh',label)
            self.assertEqual(cplane[0]['result'],'Success' if v['object_accepted'] else 'Cancel',label)
            if v['object_accepted']:
                self.assertEqual(events[0]['result'],'Success',label)
                sources = v['before']; outputs = v['after'][-4:]
                for source,output in zip(sources,outputs):
                    for actual,expected in zip(output['point'],mapped(source['point'],v)):
                        self.assertAlmostEqual(actual,expected,delta=1e-9,msg=label)
                if op['target'] in ('nonplanar_curve','warped_surface'):
                    self.assertFalse(v['sdk_plane']['available'],label); nonplanar += 1
                if op['target']=='point':
                    self.assertEqual(v['cplane']['x_axis'],v['active_plane']['x_axis'],label)
                    self.assertEqual(v['cplane']['y_axis'],v['active_plane']['y_axis'],label)
                if op['copy']:
                    self.assertEqual(len(v['after']),8,label)
                    self.assertEqual([o['role'] for o in v['after']],['source']*4+['output']*4,label)
                    before,after = copy.deepcopy(v['before']),copy.deepcopy(v['after'][:4])
                    for o in before+after: o.pop('selected')
                    self.assertEqual(before,after,label)
            else:
                self.assertEqual(events[0]['result'],'Cancel',label)
                self.assertEqual(v['cancel_inputs'],[dict(marker='@scale-object-cancel:'+label+':0')],label)
                self.assertIn('No objects added to selection.',v['history'],label)
                self.assertNotIn('First reference point',v['history'],label)
                self.assertEqual([o['point'] for o in v['before']],[o['point'] for o in v['after']],label)
                self.assertEqual(v['undo'],[],label)
            self.assertEqual(v['after_script'],v['redo'],label)
            self.assertTrue(all(not o['selected'] for o in v['after']),label)
        self.assertEqual((accepted,rejected,copies,nonplanar),(56,8,8,12))

    def test_closed_recipes_and_owned_idle_guards(self):
        q = request(); op = q['operations'][0]; validate_request(q)
        for change in (dict(op='Delete'),dict(script='_Delete'),dict(id='x\n_Delete'),dict(id=[]),
                       dict(target=[]),dict(target='unknown'),dict(plane=[]),dict(plane='unknown'),
                       dict(cplane=[]),dict(cplane='unknown'),dict(copy=1),dict(reverse=0)):
            with self.subTest(change=change),self.assertRaises(ValueError):
                validate_request(dict(protocol_version=1,operations=[dict(op,**change)]))
        for bad in (None,[],dict(protocol_version=True,operations=[op]),
                    dict(protocol_version=1,iterations=True,operations=[op]),
                    dict(protocol_version=1,iterations=2,operations=[op]),
                    dict(protocol_version=1,operations=[]),dict(protocol_version=1,operations=[op,op]),
                    dict(protocol_version=1,operations=[dict(op,id=str(i)) for i in range(65)])):
            with self.assertRaises(ValueError): validate_request(bad)
        rhino = mock.Mock(); rhino.Commands.Command.InCommand.return_value = True
        with self.assertRaisesRegex(ValueError,'idle execution'): run(op,dict(Rhino=rhino,System=mock.Mock()))
        rhino.Commands.Command.InCommand.return_value = False; rhino.RhinoDoc.ActiveDoc.Objects = [object()]
        with self.assertRaisesRegex(ValueError,'empty owned document'): run(op,dict(Rhino=rhino,System=mock.Mock()))

    def test_private_guard_precedes_launch(self):
        for scheme,display,marker in ((None,':301',':301'),('VibocerosOracleObject',':1',None),
                                     ('VibocerosOracleObject',':301',':302')):
            env = dict(DISPLAY=display)
            if marker: env['VIBOCEROS_ORACLE_HEADLESS'] = marker
            with mock.patch.dict(os.environ,env,clear=True),mock.patch('tools.rhino_oracle.client._run_logged') as launch:
                with self.assertRaises((OracleError,OracleProtocolError)):
                    OracleClient(settings_scheme=scheme).run_rhino(request(),1)
                launch.assert_not_called()

    def test_only_recipe_mesh_markers_can_send_escape_in_order(self):
        picker = ScaleByPlaneObjectPicker(request())
        with mock.patch('tools.rhino_oracle.scale_by_plane_object_input.subprocess.run') as key:
            for bad in ('@scale-object-cancel:plane-object-0:0', '@scale-object-cancel:foreign:0',
                        '@scale-object-cancel:plane-object-32:2', '@scale-object-cancel:plane-object-32:1'):
                with self.assertRaises(OracleProtocolError): picker.send_input(bad,'1','1','owned')
            key.assert_not_called()
            self.assertTrue(picker.send_input('@scale-object-cancel:plane-object-32:0','1','1','owned'))
            self.assertEqual(key.call_args.args[0],['xdotool','windowactivate','--sync','owned','key','--clearmodifiers','Escape'])

    def test_provenance(self):
        p = json.loads((ROOT/'docs/scale-by-plane-object-provenance.json').read_text())
        self.assertTrue(p['private_xvfb']); self.assertFalse(p['full_native_parity'])
        self.assertEqual((p['native_recipes'],p['accepted_targets'],p['rejected_targets'],p['application_replays']),
                         (64,56,8,120))
        for path,sha in p['sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(),sha,path)
