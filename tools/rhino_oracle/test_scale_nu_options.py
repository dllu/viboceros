"""Native option, tight-bound positioning, group and preference evidence."""
import copy
import hashlib
import json
import os
from pathlib import Path
from unittest import TestCase, mock

from .client import OracleClient, OracleError, OracleProtocolError
from .scale_nu_options_probe import MODES, request, run, validate_request

ROOT = Path(__file__).resolve().parents[2]


def dot(a,b): return sum(x*y for x,y in zip(a,b))


def positions(obj):
    if 'point' in obj: return [obj['point']]
    if 'mesh' in obj: return obj['mesh']['vertices']
    return [c['point'] for c in obj.get('curve',obj.get('surface'))['control_points']]


class ScaleNuOptionTests(TestCase):
    def captures(self):
        fixture = json.loads((ROOT/'tools/rhino_oracle/fixtures/scale_nu_options.json').read_text())
        observed = json.loads((ROOT/'tools/rhino_oracle/observations/scale_nu_options.json').read_text())
        self.assertEqual(fixture,request()); validate_request(fixture)
        self.assertEqual(len(fixture['operations']),36)
        self.assertEqual([op['id'] for op in fixture['operations']],[row['id'] for row in observed['results']])
        return list(zip(fixture['operations'],observed['results']))

    def test_closed_recipes_and_owned_document(self):
        q = request(); validate_request(q)
        op = q['operations'][0]
        for change in (dict(op='Delete'),dict(id='bad\n_Delete'),dict(id=[]),dict(id='λ'),
                       dict(script='_Delete'),dict(source=[]),dict(source='unknown'),dict(plane=[]),
                       dict(mode=[]),dict(mode='unknown'),dict(copy=1),dict(selection=[]),
                       dict(selection='grips'),dict(groups=[]),dict(groups='pair')):
            with self.subTest(change=change),self.assertRaises(ValueError):
                validate_request(dict(protocol_version=1,operations=[dict(op,**change)]))
        for bad in (None,[],dict(protocol_version=True,operations=[op]),
                    dict(protocol_version=1,iterations=True,operations=[op]),
                    dict(protocol_version=1,iterations=2,operations=[op]),
                    dict(protocol_version=1,operations=[]),dict(protocol_version=1,operations=[op,op]),
                    dict(protocol_version=1,operations=[dict(op,id=str(i)) for i in range(65)])):
            with self.assertRaises(ValueError): validate_request(bad)
        rhino = mock.Mock(); rhino.Commands.Command.InCommand.return_value = True
        with self.assertRaisesRegex(ValueError,'idle execution'): run(op,{'Rhino':rhino,'System':mock.Mock()})
        rhino.Commands.Command.InCommand.return_value = False; rhino.RhinoDoc.ActiveDoc.Objects = [object()]
        with self.assertRaisesRegex(ValueError,'empty owned document'): run(op,{'Rhino':rhino,'System':mock.Mock()})

    def test_private_launch_guard(self):
        for scheme,iterations,display,marker in ((None,1,':301',':301'),
                ('VibocerosOracleOptions',True,':301',':301'),
                ('VibocerosOracleOptions',2,':301',':301'),
                ('VibocerosOracleOptions',1,':1',None),
                ('VibocerosOracleOptions',1,':301',':302')):
            env = {'DISPLAY':display}
            if marker: env['VIBOCEROS_ORACLE_HEADLESS'] = marker
            with mock.patch.dict(os.environ,env,clear=True),mock.patch('tools.rhino_oracle.client._run_logged') as launch:
                with self.assertRaises((OracleError,OracleProtocolError,ValueError)):
                    OracleClient(settings_scheme=scheme).run_rhino(dict(request(),iterations=iterations),1)
                launch.assert_not_called()

    def test_complete_geometry_follows_prescribed_frames_and_tight_centers(self):
        for op,row in self.captures():
            v = row['value']; before = v['before']['objects']; label = op['id']
            self.assertTrue(v['success'],label)
            self.assertEqual(v['after'],v['after_script'],label)
            expected_redo = copy.deepcopy(v['after'])
            if op['mode']=='rigid_yes' and not op['copy'] and op['selection']!='objects':
                for obj in expected_redo['objects']:
                    original = next(o for o in before if o['name']==obj['name'])
                    obj['grips'] = copy.deepcopy(original['grips'])
            self.assertEqual(expected_redo,v['redo'],label)
            basis = [v['plane']['x_axis'],v['plane']['y_axis']]
            a,b = basis
            basis.append([a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0]])
            if op['mode']=='world_yes': basis = [[1.,0.,0.],[0.,1.,0.],[0.,0.,1.]]
            rigid = op['mode'] in ('rigid_yes','rigid_y','rigid_repeat','remember_rigid','remember_rigid_cancel','rigid_zero')
            factors = [2.,3.,.5] if op['mode'].startswith('world') or op['mode'] in ('remember_world','remember_factors_cancel') else [2.,-1.,.5]
            if op['mode']=='rigid_zero': factors = [0.,1.,1.]
            factor_sets = [factors,[4.,.5,1.]] if op['mode']=='rigid_repeat' else [factors]
            def mapping(p,factors):
                delta = [p[0]-1.,p[1]-2.,p[2]-3.]
                return [origin+sum(f*dot(delta,axis)*axis[i] for axis,f in zip(basis,factors)) for i,origin in enumerate([1.,2.,3.])]
            ignored = {0} if rigid and op['selection']!='objects' else set()
            def center(i):
                groups = [members for members in v['before']['groups'] if i in members]
                members = [j for j in groups[-1] if j not in ignored] if groups else [i]
                boxes = [v['bounds'][j]['tight'] for j in members]
                return [(min(b['min'][axis] for b in boxes)+max(b['max'][axis] for b in boxes))/2. for axis in range(3)]
            for i in ignored:
                actual = next(o for o in v['after']['objects'] if o['name']==before[i]['name'])
                expected = copy.deepcopy(before[i])
                if not op['copy']:
                    for grip in expected['grips']:
                        if grip['selected']: grip['point'] = mapping(grip['point'],[factors[0],factors[1],1.])
                for got,wanted in zip(actual['grips'],expected['grips']):
                    for x,y in zip(got['point'],wanted['point']): self.assertAlmostEqual(x,y,delta=1e-9,msg=label)
                    wanted['point'] = got['point']
                self.assertEqual(actual,expected,label)
            for i,source in enumerate(before):
                if i in ignored: continue
                outputs = [o for o in v['after']['objects'] if o['name']==source['name'] and (o['role']=='output')==op['copy']]
                self.assertEqual(len(outputs),len(factor_sets),label)
                for result,factors in zip(outputs,factor_sets):
                    displacement = [x-y for x,y in zip(mapping(center(i),factors),center(i))] if rigid else None
                    for p,q in zip(positions(source),positions(result)):
                        expected = [x+y for x,y in zip(p,displacement)] if rigid else mapping(p,factors)
                        for actual,wanted in zip(q,expected): self.assertAlmostEqual(actual,wanted,delta=1e-9,msg=label)
                    for field in ('curve','surface'):
                        if field not in source: continue
                        a,b = copy.deepcopy(source[field]),copy.deepcopy(result[field])
                        for item in (a,b):
                            for control in item['control_points']: control.pop('point')
                        self.assertEqual(a,b,label)

    def test_grip_owner_geometry_is_ignored_and_memory_follows_native_phases(self):
        rows = dict((op['id'],row['value']) for op,row in self.captures())
        for i in (21,22,32,33):
            v = rows['scale-nu-options-'+str(i)]
            before,after = copy.deepcopy(v['before']),copy.deepcopy(v['after'])
            for o in before['objects']+after['objects']: o.pop('grips')
            self.assertEqual(before,after)
            if i in (21,22): self.assertEqual(v['before'],v['after'])
            else: self.assertNotEqual(v['before'],v['after'])
            self.assertEqual(v['undo']['objects'],[])
        for i in (27,34):
            v = rows['scale-nu-options-'+str(i)]
            self.assertIn('WorldCoordinates=No  Rigid=Yes',v['history'])
        self.assertIn('WorldCoordinates=No',rows['scale-nu-options-28']['history'])
        v = rows['scale-nu-options-35']
        self.assertIn('7 _Cancel',v['seed_macros'][-1])
        for axis,default in (('X','2.000'),('Y','3.000'),('Z','0.500')):
            self.assertIn(axis+' axis scale or first reference point <'+default+'>',v['history'])

    def test_next_move_uses_geometry_and_discards_temporary_grip_locations(self):
        fixture = json.loads((ROOT/'tools/rhino_oracle/fixtures/scale_nu_pending_grips.json').read_text())
        observed = json.loads((ROOT/'tools/rhino_oracle/observations/scale_nu_pending_grips.json').read_text())
        validate_request(fixture)
        self.assertEqual(len(fixture['operations']),3)
        self.assertEqual([op['id'] for op in fixture['operations']],[row['id'] for row in observed['results']])
        for row in observed['results']:
            v = row['value']; label = row['id']
            self.assertTrue(v['followup']['success'],label)
            self.assertEqual(v['followup']['macro'],'_Move w0,0,0 w1,2,3')
            self.assertEqual(v['before'],v['undo'],label)
            self.assertEqual(v['followup']['after'],v['redo'],label)
            before,after = v['before']['objects'][0],v['after']['objects'][0]
            for field in ('curve','surface','mesh'):
                if field in before: self.assertEqual(before[field],after[field],label)
            moved = v['followup']['after']['objects'][0]
            for grip,result in zip(before['grips'],moved['grips']):
                expected = [x+y for x,y in zip(grip['point'],[1.,2.,3.])] if grip['selected'] else grip['point']
                self.assertEqual(result['point'],expected,label)
            expected = copy.deepcopy(before)
            for grip in expected['grips']:
                if grip['selected']: grip['point'] = [x+y for x,y in zip(grip['point'],[1.,2.,3.])]
            for i,p in enumerate(positions(expected)):
                grip_index = i
                if 'surface' in expected:
                    u,v_count = expected['surface']['control_count']
                    grip_index = (i % u) * v_count + i // u
                if before['grips'][grip_index]['selected']:
                    p[:] = [x+y for x,y in zip(p,[1.,2.,3.])]
            self.assertEqual(expected,moved,label)

    def test_provenance(self):
        provenance = json.loads((ROOT/'docs/scale-nu-options-provenance.json').read_text())
        self.assertTrue(provenance['private_xvfb']); self.assertFalse(provenance['full_native_parity'])
        self.assertEqual(provenance['native_recipes'],36)
        self.assertEqual(provenance['application_replays'],72)
        self.assertEqual(provenance['followup_move_recipes'],3)
        self.assertEqual(provenance['followup_move_application_replays'],6)
        for path,expected in provenance['sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(),expected,path)
