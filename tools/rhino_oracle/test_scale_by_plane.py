"""Public command evidence, independent plane maps, and private input guards."""
import copy
import hashlib
import json
import os
import tempfile
from pathlib import Path
from unittest import TestCase, mock

from .client import OracleClient, OracleError, OracleProtocolError
from .scale_by_plane_input import ScaleByPlaneViewPicker
from .scale_by_plane_probe import boundary_request, request, run, validate_request
from .test_scale_nu_options import dot, positions

ROOT = Path(__file__).resolve().parents[2]


def plane_map(point, value):
    # FromView is independently distinguished from camera-frame scaling by
    # prescribed world reference points and the clicked viewport's SDK CPlane.
    plane = value['active_plane'] if value['view_clicked'] else value['plane']
    origin = value['origin']
    delta = [x-y for x,y in zip(point, origin)]
    result = list(point)
    for key in ('x_axis', 'y_axis'):
        axis = plane[key]
        denominator = dot([x-y for x,y in zip(value['reference'], origin)], axis)
        numerator = dot([x-y for x,y in zip(value['target'], origin)], axis)
        factor = numerator/denominator if abs(denominator)>1e-6 and abs(numerator)>1e-6 else 1.
        result = [x+(factor-1.)*dot(delta,axis)*u for x,u in zip(result,axis)]
    return result


class ScaleByPlaneTests(TestCase):
    def captures(self):
        rows = []
        for suffix, factory, count in (('',request,58),('_boundary',boundary_request,48)):
            q = json.loads((ROOT/('tools/rhino_oracle/fixtures/scale_by_plane'+suffix+'.json')).read_text())
            r = json.loads((ROOT/('tools/rhino_oracle/observations/scale_by_plane'+suffix+'.json')).read_text())
            self.assertEqual(q,factory()); validate_request(q)
            self.assertEqual(len(q['operations']),count)
            self.assertEqual([op['id'] for op in q['operations']],[row['id'] for row in r['results']])
            rows.extend(zip(q['operations'],r['results']))
        return rows

    def test_independent_signed_maps_rigid_centers_and_selected_grips(self):
        for op,row in self.captures():
            v, label = row['value'],op['id']
            source = v['before'][0]
            output = v['after'][-1]
            selected = {g['index'] for g in source['grips'] if g['selected']}
            if op['rigid']:
                displacement = [0.,0.,0.] if selected else [x-y for x,y in zip(plane_map(source['center'],v),source['center'])]
                expected = [[x+d for x,d in zip(point,displacement)] for point in positions(source)]
            else:
                # Surface grip order is v-major; SDK surface controls u-major.
                count_v = source.get('surface',{}).get('control_count',[0,0])[1]
                count_u = source.get('surface',{}).get('control_count',[0,0])[0]
                expected = []
                for i,point in enumerate(positions(source)):
                    grip = i%count_v*count_u+i//count_v if count_v else i
                    expected.append(plane_map(point,v) if not selected or grip in selected else point)
            for actual, target in zip(positions(output),expected):
                for a,b in zip(actual,target): self.assertAlmostEqual(a,b,delta=1e-9,msg=label)
            a,b = copy.deepcopy(source),copy.deepcopy(output)
            for obj in (a,b):
                for key in ('role','selected','center','grips','grips_on'): obj.pop(key)
                for point in positions(obj): point[:] = [0.,0.,0.]
            # Nonuniform analytic circles/arcs are converted to NURBS.
            if op['source'] in ('circle','arc') and not op['rigid']: a['kind'] = 'NurbsCurve'
            self.assertEqual(a,b,label)
            if op['copy']:
                before,after = copy.deepcopy(v['before']),copy.deepcopy(v['after'][:1])
                for obj in before+after: obj.pop('selected')
                self.assertEqual(before,after,label)

    def test_native_success_view_acknowledgement_and_external_history(self):
        for op,row in self.captures():
            v,label = row['value'],op['id']
            events = [e for e in v['events'] if e['name']=='ScaleByPlane']
            self.assertEqual(len(events),1,label)
            self.assertEqual(events[0]['result'],'Success',label)
            self.assertTrue(v['success'],label)
            self.assertEqual(events[0]['objects'],v['after'],label)
            idle,redo = copy.deepcopy(v['after_script']),copy.deepcopy(v['redo'])
            if op['rigid'] and op['selection']=='parent':
                for obj in idle+redo:
                    for grip in obj['grips']: grip.pop('point')
            self.assertEqual(idle,redo,label)
            if op['plane']=='FromView':
                self.assertEqual(len(v['view_pick']),1,label)
                self.assertEqual(len(v['view_clicked']),1,label)
                self.assertEqual(v['view_pick'][0]['view'],v['view_clicked'][0]['view'],label)
                self.assertEqual(v['active_plane'],v['view_clicked'][0]['construction_plane'],label)
            else:
                self.assertEqual(v['view_pick'],[],label)
                self.assertEqual(v['view_clicked'],[],label)
            if op['rigid'] and op['selection']=='parent': self.assertEqual(v['undo'],[],label)
            if op['copy']:
                self.assertEqual(len(v['after']),2,label)
                self.assertEqual(len(v['undo']),1,label)
            self.assertIn('Plane=ActiveCPlane',v['history'],label)

    def test_closed_recipes_idle_and_empty_document_guards(self):
        q = request(); validate_request(q); op = q['operations'][0]
        for change in (dict(op='Delete'),dict(script='_Delete'),dict(id='x\n_Delete'),dict(id=[]),
                       dict(source=[]),dict(source='unknown'),dict(plane=[]),dict(plane='unknown'),
                       dict(input=[]),dict(view=[]),dict(cplane=[]),dict(copy=1),dict(rigid=0),
                       dict(selection='grips')):
            with self.subTest(change=change),self.assertRaises(ValueError):
                validate_request(dict(protocol_version=1,operations=[dict(op,**change)]))
        for bad in (None,[],dict(protocol_version=True,operations=[op]),dict(protocol_version=1,iterations=True,operations=[op]),
                    dict(protocol_version=1,iterations=2,operations=[op]),dict(protocol_version=1,operations=[]),
                    dict(protocol_version=1,operations=[op,op]),dict(protocol_version=1,operations=[dict(op,id=str(i)) for i in range(65)])):
            with self.assertRaises(ValueError): validate_request(bad)
        rhino = mock.Mock(); rhino.Commands.Command.InCommand.return_value = True
        with self.assertRaisesRegex(ValueError,'idle execution'): run(op,dict(Rhino=rhino,System=mock.Mock()))
        rhino.Commands.Command.InCommand.return_value = False; rhino.RhinoDoc.ActiveDoc.Objects = [object()]
        with self.assertRaisesRegex(ValueError,'empty owned document'): run(op,dict(Rhino=rhino,System=mock.Mock()))

    def test_private_launch_guard_precedes_process_and_input(self):
        for scheme,display,marker in ((None,':301',':301'),('VibocerosOraclePlane',':1',None),('VibocerosOraclePlane',':301',':302')):
            env = dict(DISPLAY=display)
            if marker: env['VIBOCEROS_ORACLE_HEADLESS'] = marker
            with mock.patch.dict(os.environ,env,clear=True),mock.patch('tools.rhino_oracle.client._run_logged') as launch:
                with self.assertRaises((OracleError,OracleProtocolError)): OracleClient(settings_scheme=scheme).run_rhino(request(),1)
                launch.assert_not_called()

    def test_view_click_waits_for_native_motion_in_owned_window(self):
        q = request(); q['operations'] = [op for op in q['operations'] if op['plane']=='FromView'][:1]
        picker = ScaleByPlaneViewPicker(q); name = next(iter(picker.cases))
        with tempfile.TemporaryDirectory() as temp,mock.patch('tools.rhino_oracle.scale_by_plane_input.subprocess.run') as input_call:
            picker.job = Path(temp)
            self.assertFalse(picker.send_input(name,'100','200','owned'))
            self.assertNotIn('click',input_call.call_args.args[0])
            picker.moved[name] = ('owned','100','200',picker.moved[name][3]-.3)
            self.assertFalse(picker.send_input(name,'100','200','owned'))
            ready = picker.job/('scale-by-plane-view-ready-'+picker.cases[name]+'.json')
            ready.write_text(json.dumps('foreign'))
            with self.assertRaises(OracleProtocolError): picker.send_input(name,'100','200','owned')
            ready.write_text(json.dumps(name))
            with self.assertRaises(OracleProtocolError): picker.send_input(name,'101','200','owned')
            self.assertTrue(picker.send_input(name,'100','200','owned'))
            self.assertIn('click',input_call.call_args.args[0])

    def test_provenance(self):
        p = json.loads((ROOT/'docs/scale-by-plane-provenance.json').read_text())
        self.assertTrue(p['private_xvfb']); self.assertFalse(p['full_native_parity'])
        self.assertEqual(p['native_recipes'],106)
        self.assertEqual(p['application_replays'],212)
        self.assertEqual(p['known_display_exclusions'],['scale-by-plane-'+str(i) for i in (52,53,54)])
        for path,sha in p['sha256'].items(): self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(),sha,path)
