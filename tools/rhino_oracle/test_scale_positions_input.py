"""Independent placement equations and command-first input/history evidence."""
import copy
import hashlib
import json
import math
from unittest import TestCase

from .scale_positions_probe import input_request, validate_request
from .test_scale_positions import ROOT
from .test_scale_nu_options import dot, positions


def placement(op):
    mode, kind = op['mode'], op['input']
    if kind == 'repeat_reference':
        return mode, [2., .5 if mode == '1d' else math.sqrt(1.5)]
    if kind in ('reference_numeric', 'reference_numeric_negative'):
        return mode, [] if mode == '1d' else [9.]
    if kind == 'remember_reference_numeric':
        return mode, [2. if mode == '1d' else 9.]
    if kind == 'remember_reference_cancel_mode':
        return '3d', [2.]
    if kind in ('reference_numeric_zero', 'zero_reference_copy_cancel',
                'cancel_origin', 'cancel_sources', 'remember_repeat_switch_mode',
                'remember_numeric_cancel_mode'):
        return mode, []
    return mode, [1. if kind == 'identity' else 2.]


def preferences(op):
    mode, kind = op['mode'], op['input']
    factor, copy_option = 2., op['copy']
    if kind == 'repeat_reference':
        factor = .5 if mode == '1d' else math.sqrt(1.5)
    elif kind in ('reference_numeric', 'reference_numeric_negative', 'remember_reference_numeric'):
        factor = 2. if mode == '1d' else 9.
    elif kind == 'identity': factor = 1. if mode == '1d' else 2.
    elif kind in ('repeat_switch_mode', 'remember_repeat_switch_mode'):
        mode, copy_option = '1d', True
    elif kind == 'remember_reference_cancel_mode': mode = '3d'
    elif kind == 'remember_numeric_cancel_mode': mode, copy_option = '1d', False
    return mode, factor, copy_option


def displacement(center, origin, mode, factor, plane, direction=(2., 4., 2.)):
    delta = [x-y for x,y in zip(center, origin)]
    if mode == '1d':
        return [(factor-1.)*dot(delta,direction)*x/dot(direction,direction) for x in direction]
    if mode == '2d':
        a,b = plane['x_axis'],plane['y_axis']
        normal = [a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0]]
        return [(factor-1.)*(x-dot(delta,normal)*n) for x,n in zip(delta,normal)]
    return [(factor-1.)*x for x in delta]


class ScalePositionsInputTests(TestCase):
    def captures(self):
        q = json.loads((ROOT/'tools/rhino_oracle/fixtures/scale_positions_input.json').read_text())
        r = json.loads((ROOT/'tools/rhino_oracle/observations/scale_positions_input.json').read_text())
        self.assertEqual(q, input_request()); validate_request(q)
        self.assertEqual(len(q['operations']),64)
        self.assertEqual([op['id'] for op in q['operations']],[row['id'] for row in r['results']])
        return list(zip(q['operations'],r['results']))

    def test_complete_geometry_order_groups_and_metadata(self):
        for op,row in self.captures():
            v, label = row['value'], op['id']
            sources = v['before']['objects']
            mode, factors = placement(op)
            order = v.get('source_selection',list(range(len(sources))))
            changed = bool(factors) and (op['copy'] or factors != [1.])
            expected_order = order if changed and not op['copy'] else list(range(len(sources)))
            if op['copy']:
                expected_order += order*len(factors)
            self.assertEqual([o['name'] for o in v['after']['objects']],
                             [sources[i]['name'] for i in expected_order],label)
            for i,source in enumerate(sources):
                outputs = [o for o in v['after']['objects'] if o['name']==source['name'] and
                           (o['role']=='output')==op['copy']]
                expected_factors = factors if op['copy'] else factors or [1.]
                self.assertEqual(len(outputs),len(expected_factors),label)
                for output,factor in zip(outputs,expected_factors):
                    shift = displacement(v['bounds'][i]['tight']['center'],[1.,2.,3.],mode,factor,v['plane'])
                    a,b = copy.deepcopy(source),copy.deepcopy(output)
                    for p,q in zip(positions(a),positions(b)):
                        for x,y,d in zip(p,q,shift): self.assertAlmostEqual(y,x+d,delta=1e-9,msg=label)
                        q[:] = p
                    for obj in (a,b):
                        obj.pop('role'); obj.pop('selected')
                    self.assertEqual(a,b,label)
            for old,new in zip(v['before']['groups'],v['after']['groups']):
                members = {sources[i]['name'] for i in old}
                self.assertEqual(new,[i for i,obj in enumerate(v['after']['objects']) if obj['name'] in members],label)

    def test_command_first_selection_cleanup_cancellation_and_history(self):
        for op,row in self.captures():
            v,label = row['value'],op['id']
            mode,factors = placement(op)
            postselected = op['selection'].startswith('post_')
            changed = bool(factors) and (op['copy'] or factors != [1.])
            canceled = op['input'] in ('cancel_origin','cancel_sources','repeat_switch_mode','remember_numeric_cancel_mode')
            self.assertEqual(v['success'],not canceled,label)
            event = next(e for e in v['events'] if e['name']=='ScalePositions')
            self.assertEqual(event['result'],'Cancel' if canceled else 'Success',label)
            if postselected:
                self.assertFalse(any(o['selected'] for o in v['before']['objects']),label)
                self.assertFalse(any(o['selected'] for o in v['after_script']['objects']),label)
                picked = not (changed and not op['copy']) and op['input']!='cancel_sources'
                self.assertEqual([o['selected'] for o in v['after']['objects']],
                                 [picked and o['role']=='source' for o in v['after']['objects']],label)
            self.assertEqual(v['after_script'],v['redo'],label)
            if changed:
                self.assertEqual(v['undo'],v['before'],label)
            else:
                self.assertEqual(v['undo']['objects'],[],label)
                self.assertEqual(v['undo']['groups'],[[] for _ in v['before']['groups']],label)

    def test_completed_defaults_and_canceled_option_lifetimes(self):
        for op,row in self.captures():
            v,label = row['value'],op['id']
            mode,factor,copy_option = preferences(op)
            self.assertEqual(v['preferences']['mode'],mode.upper(),label)
            self.assertEqual(v['preferences']['copy'],copy_option,label)
            self.assertAlmostEqual(v['preferences']['factor'],factor,delta=5e-6,msg=label)
            self.assertIn('Mode='+mode.upper(),v['preference_history'],label)
            self.assertIn('<'+v['preferences']['factor_text']+'>',v['preference_history'],label)
            # Two/three-dimensional Enter witnesses retain the precision
            # omitted from the displayed noninteger factor. One-dimensional
            # witness diagnostics are kept raw while their inconsistency is
            # investigated; their actual recipe geometry is checked above.
            if mode != '1d':
                witness = v['default_witness']
                shift = displacement(witness['before'],[0.,0.,0.],mode,factor,v['plane'])
                for x,y,d in zip(witness['before'],witness['after'],shift):
                    self.assertAlmostEqual(y,x+d,delta=1e-9,msg=label)

    def test_new_recipes_remain_closed_and_bounded(self):
        q = input_request(); validate_request(q)
        for op in q['operations']:
            for change in (dict(script='_Delete'),dict(id='bad\n_Delete'),dict(copy=1)):
                with self.assertRaises(ValueError):
                    validate_request(dict(protocol_version=1,operations=[dict(op,**change)]))
        for index,change in ((33,dict(source='rational')),(35,dict(selection='objects'))):
            with self.assertRaises(ValueError):
                validate_request(dict(protocol_version=1,operations=[dict(q['operations'][index],**change)]))

    def test_refresh_retains_inconsistent_1d_history_as_diagnostics(self):
        original = json.loads((ROOT/'tools/rhino_oracle/observations/scale_positions.json').read_text())
        diagnostic = json.loads((ROOT/'tools/rhino_oracle/observations/scale_positions_defaults_diagnostic.json').read_text())
        self.assertEqual(len(diagnostic['results']),57)
        self.assertEqual([r['id'] for r in original['results']],[r['id'] for r in diagnostic['results']])
        a,b = [capture['results'][6]['value'] for capture in (original,diagnostic)]
        self.assertEqual(a['macro'],b['macro']); self.assertEqual(a['before'],b['before'])
        for p,q in zip(a['after']['objects'][0]['point'],b['after']['objects'][0]['point']):
            self.assertAlmostEqual(p,q,delta=1e-9)
        self.assertEqual(len(a['undo']['objects']),1)
        self.assertEqual(b['undo']['objects'],[])

    def test_provenance(self):
        p = json.loads((ROOT/'docs/scale-positions-input-provenance.json').read_text())
        self.assertTrue(p['private_xvfb']); self.assertFalse(p['full_native_parity'])
        self.assertEqual(p['native_recipes'],64); self.assertEqual(p['application_replays'],64)
        self.assertEqual(p['diagnostic_recipes'],57); self.assertEqual(p['diagnostic_application_replays'],0)
        self.assertEqual(p['geometry_absolute_epsilon'],1e-9)
        for path,digest in p['sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(),digest,path)
