"""Independent center maps, bounded capture guards, and retained native states."""
import copy
import hashlib
import json
import os
from pathlib import Path
from unittest import TestCase, mock

from .client import OracleClient, OracleError, OracleProtocolError
from .scale_positions_probe import request, run, validate_request
from .test_scale_nu_options import dot, positions

ROOT = Path(__file__).resolve().parents[2]


class ScalePositionsTests(TestCase):
    def captures(self):
        fixture = json.loads((ROOT/'tools/rhino_oracle/fixtures/scale_positions.json').read_text())
        observed = json.loads((ROOT/'tools/rhino_oracle/observations/scale_positions.json').read_text())
        self.assertEqual(fixture,request()); validate_request(fixture)
        self.assertEqual(len(fixture['operations']),57)
        self.assertEqual([op['id'] for op in fixture['operations']],[row['id'] for row in observed['results']])
        return list(zip(fixture['operations'],observed['results']))

    def test_complete_geometry_uses_individual_tight_centers_and_preserves_shapes(self):
        for op,row in self.captures():
            v = row['value']; label = op['id']; before = v['before']['objects']
            after_script = copy.deepcopy(v['after'])
            if op['input']=='zero_reference_cancel' or op['input']=='remember_zero_reference' and op['mode']!='1d':
                # The final Cancel runs at idle after successful completion and
                # clears selection; EndCommand precedes this separate action.
                for obj in after_script['objects']:
                    obj['selected'] = False
                    for grip in obj['grips']: grip['selected'] = False
            self.assertEqual(after_script,v['after_script'],label)
            self.assertEqual(v['after_script'],v['redo'],label)
            canceled = op['input']=='zero_cancel' or op['selection']=='grips' and op['source']!='mixed'
            self.assertEqual(v['success'],not canceled,label)
            self.assertEqual(v['events'][-1]['result'],'Cancel' if canceled else 'Success',label)
            noedit = canceled or op['input'] in ('identity','change_mode_from_3d','zero_reference_cancel') or op['input']=='remember_zero_reference' and op['mode']=='1d'
            if noedit:
                a,b = copy.deepcopy(v['before']),copy.deepcopy(v['after'])
                if op['selection']=='grips':
                    for o in a['objects']:
                        for g in o['grips']: g['selected'] = False
                self.assertEqual(a,b,label)
                self.assertEqual(v['undo']['objects'],[],label)
                continue
            self.assertEqual(v['undo'],v['before'],label)
            normal = [v['plane']['x_axis'][1]*v['plane']['y_axis'][2]-v['plane']['x_axis'][2]*v['plane']['y_axis'][1],
                      v['plane']['x_axis'][2]*v['plane']['y_axis'][0]-v['plane']['x_axis'][0]*v['plane']['y_axis'][2],
                      v['plane']['x_axis'][0]*v['plane']['y_axis'][1]-v['plane']['x_axis'][1]*v['plane']['y_axis'][0]]
            directions = [[2.,4.,2.],[4.,0.,0.]] if op['input']=='repeat_direction' else [[2.,4.,2.]]
            factors = [2.,3.] if op['input']=='repeat_factor' else [2.]
            if op['input']=='offaxis_reference': factors = [.5 if op['mode']=='1d' else (1.5)**.5]
            for i,source in enumerate(before):
                if op['selection']=='grips' and i==0: continue
                center = v['bounds'][i]['tight']['center']
                delta = [x-y for x,y in zip(center,[1.,2.,3.])]
                shifts = []
                for iteration in range(max(len(factors),len(directions))):
                    factor = factors[min(iteration,len(factors)-1)]
                    if op['mode']=='1d':
                        axis = directions[min(iteration,len(directions)-1)]
                        shift = [(factor-1.)*dot(delta,axis)*x/dot(axis,axis) for x in axis]
                    elif op['mode']=='2d':
                        shift = [(factor-1.)*(x-dot(delta,normal)*n) for x,n in zip(delta,normal)]
                    else: shift = [(factor-1.)*x for x in delta]
                    shifts.append(shift)
                outputs = [o for o in v['after']['objects'] if o['name']==source['name'] and (o['role']=='output')==op['copy']]
                self.assertEqual(len(outputs),len(shifts),label)
                for output,shift in zip(outputs,shifts):
                    self.assertEqual(source['attribute_user_text'],output['attribute_user_text'],label)
                    self.assertEqual(source['geometry_user_text'],output['geometry_user_text'],label)
                    for p,q in zip(positions(source),positions(output)):
                        for x,y,d in zip(p,q,shift): self.assertAlmostEqual(y,x+d,delta=1e-9,msg=label)
                    for field in ('curve','surface'):
                        if field not in source: continue
                        a,b = copy.deepcopy(source[field]),copy.deepcopy(output[field])
                        for item in (a,b):
                            for control in item['control_points']: control.pop('point')
                        self.assertEqual(a,b,label)
            self.assertEqual(len(v['after']['groups']),len(v['before']['groups']),label)
            for original,result in zip(v['before']['groups'],v['after']['groups']):
                self.assertTrue(set(original).issubset(result),label)

    def test_copy_joins_original_groups_and_keeps_parent_grip_display(self):
        rows = {op['id']:row['value'] for op,row in self.captures()}
        self.assertEqual(rows['scale-positions-31']['after']['groups'],[[0,1,4,5],[1,3,5,7]])
        self.assertEqual(rows['scale-positions-44']['after']['groups'],[[0,1,4,5,8,9]])
        v = rows['scale-positions-47']; original,result = v['after']['objects']
        self.assertTrue(result['grips_on']); self.assertFalse(result['selected'])
        self.assertEqual([g['selected'] for g in original['grips']],[g['selected'] for g in result['grips']])
        for source,target in zip(original['grips'],result['grips']):
            for p,q,d in zip(source['point'],target['point'],[0.,-1.,-2.5]): self.assertAlmostEqual(q,p+d,delta=1e-9)

    def test_canceled_scalar_and_mode_do_not_replace_completed_defaults(self):
        rows = {op['id']:row['value'] for op,row in self.captures()}
        v = rows['scale-positions-45']
        self.assertIn('7 _Cancel',v['seed_macros'][-1])
        self.assertIn('reference point <2>',v['history'])
        self.assertEqual(v['seed_events'][-1][-1]['result'],'Success')
        self.assertIn('Mode=1D',rows['scale-positions-46']['history'])
        for i in (48,49,50):
            v = rows['scale-positions-'+str(i)]
            self.assertEqual('Scale direction' in v['history'],i!=48)
        for i,mode in ((54,'1D'),(55,'2D'),(56,'3D')):
            v = rows['scale-positions-'+str(i)]
            self.assertIn('Mode='+mode,v['history']); self.assertIn('reference point <2>',v['history'])

    def test_closed_request_and_owned_document_guards(self):
        q = request(); validate_request(q); op = q['operations'][0]
        for change in (dict(op='Delete'),dict(script='_Delete'),dict(id='x\n_Delete'),dict(id=[]),
                       dict(source=[]),dict(source='unknown'),dict(plane=[]),dict(mode=[]),
                       dict(input=[]),dict(input='unknown'),dict(copy=1),dict(groups='pair'),
                       dict(selection='grips'),dict(input='repeat_factor'),dict(input='change_mode_from_1d')):
            with self.subTest(change=change),self.assertRaises(ValueError): validate_request(dict(protocol_version=1,operations=[dict(op,**change)]))
        for bad in (None,dict(protocol_version=True,operations=[op]),dict(protocol_version=1,iterations=True,operations=[op]),
                    dict(protocol_version=1,iterations=2,operations=[op]),dict(protocol_version=1,operations=[]),
                    dict(protocol_version=1,operations=[op,op]),dict(protocol_version=1,operations=[dict(op,id=str(i)) for i in range(65)])):
            with self.assertRaises(ValueError): validate_request(bad)
        rhino = mock.Mock(); rhino.Commands.Command.InCommand.return_value = True
        with self.assertRaisesRegex(ValueError,'idle execution'): run(op,{'Rhino':rhino,'System':mock.Mock()})
        rhino.Commands.Command.InCommand.return_value = False; rhino.RhinoDoc.ActiveDoc.Objects = [object()]
        with self.assertRaisesRegex(ValueError,'empty owned document'): run(op,{'Rhino':rhino,'System':mock.Mock()})

    def test_private_launch_guard(self):
        for scheme,display,marker in ((None,':301',':301'),('VibocerosOraclePositions',':1',None),('VibocerosOraclePositions',':301',':302')):
            env = {'DISPLAY':display}
            if marker: env['VIBOCEROS_ORACLE_HEADLESS'] = marker
            with mock.patch.dict(os.environ,env,clear=True),mock.patch('tools.rhino_oracle.client._run_logged') as launch:
                with self.assertRaises((OracleError,OracleProtocolError,ValueError)): OracleClient(settings_scheme=scheme).run_rhino(request(),1)
                launch.assert_not_called()

    def test_provenance(self):
        provenance = json.loads((ROOT/'docs/scale-positions-provenance.json').read_text())
        self.assertTrue(provenance['private_xvfb']); self.assertFalse(provenance['full_native_parity'])
        self.assertEqual(provenance['native_recipes'],57); self.assertEqual(provenance['application_replays'],99)
        for path,digest in provenance['sha256'].items(): self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(),digest,path)
