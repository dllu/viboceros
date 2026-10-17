"""NURBS lookup, command lifecycle and bounded public SDK diagnostics."""
import copy
import json
from pathlib import Path
from unittest import TestCase
from .flow_line_comparison import comparison
from .flow_probe import validate

BASE = Path(__file__).parent


class FlowNurbsTests(TestCase):
    def read(self, path):
        return json.loads((BASE / path).read_text())

    def test_native_nurbs_maps_and_command_workflows_match_all_retained_states(self):
        for name, count in [('flow_nurbs',72),('flow_nurbs_command',72),('flow_nurbs_spatial',60)]:
            q = self.read('fixtures/'+name+'.json')
            a = self.read('observations/'+name+'_viboceros_20261017.json')
            n = self.read('observations/'+name+'_rhino_20261017.json')
            r = comparison(q,a,n)
            self.assertEqual(r,self.read('observations/'+name+'_comparison_20261017.json'))
            self.assertEqual(r['cases'],count)
            self.assertEqual(r['matched_cases'],count)
            self.assertLess(max(s['max_coordinate_error']for row in r['results']for s in row['stages']),1e-6)

    def test_finite_geometry_corruption_cannot_pass(self):
        q = self.read('fixtures/flow_nurbs.json')
        a = self.read('observations/flow_nurbs_viboceros_20261017.json')
        n = self.read('observations/flow_nurbs_rhino_20261017.json')
        a['results'][0]['value']['sdk_points'][0][0] += .01
        r = comparison(q,a,n)
        self.assertEqual(r['matched_cases'],71)

    def test_arbitrary_native_queries_confirm_sequential_prefixes_and_curve_lifetime(self):
        q = self.read('fixtures/flow_inverse_queries.json')
        n = self.read('observations/flow_inverse_queries_rhino_20261017.json')
        self.assertEqual([o['id']for o in q['operations']],[r['id']for r in n['results']])
        for op, result in zip(q['operations'],n['results']):
            validate(op)
            requests = op['diagnostic_length_requests']
            values = result['value']['diagnostics']['target']['length_requests']
            self.assertEqual(len(values),len(requests))
            full = requests[0]['fractions']
            parameters = values[0]['parameters']
            self.assertEqual(len(parameters),841)
            find = lambda fractions: next(i for i,r in enumerate(requests)if r['fractions']==fractions)
            self.assertEqual(parameters[:421],values[find(full[:421])]['parameters'])
            suffix = values[find(full[420:])]['parameters']
            self.assertGreater(abs(parameters[420]-suffix[0]),1e-5)
            self.assertIsNone(values[find(list(reversed(full)))]['parameters'])
            self.assertEqual(parameters,values[-1]['parameters'])
            self.assertFalse(requests[-1]['fresh'])
            self.assertEqual(parameters,values[-2]['parameters'])
            self.assertEqual(requests[-2]['absolute_tolerance'],1e-12)

    def test_native_snap_windows_recover_independently_integrated_checkpoints(self):
        q = self.read('fixtures/flow_inverse_nodes.json')
        n = self.read('observations/flow_inverse_nodes_rhino_20261017.json')
        independent = self.read('observations/flow_integration_viboceros_20261016.json')
        for op,result,reference in zip(q['operations'],n['results'],independent['results']):
            validate(op)
            trace = reference['value']['integration_traces']['target'][2]['checkpoints'][1:-1]
            values = result['value']['diagnostics']['target']['length_requests'][0]['parameters']
            self.assertEqual(len(values),5*len(trace))
            for i,node in enumerate(trace):
                middle = values[5*i+1:5*i+4]
                if i==0:
                    self.assertNotEqual(middle[0],middle[-1])
                else:
                    self.assertEqual(middle,[middle[0]]*3)
                    self.assertLess(abs(middle[0]-node['t']),1e-8)

    def test_public_query_validation_bounds_counts_values_and_options(self):
        original = self.read('fixtures/flow_inverse_queries.json')['operations'][0]
        original = copy.deepcopy(original)
        original['diagnostic_length_requests'] = [dict(fractions=[0.,.5,1.],absolute_tolerance=0.,fractional_tolerance=1e-4)]
        validate(original)
        for key,value in [('fractions',[]),('fractions',[True]),('fractions',[float('nan')]),('fractions',[float('inf')]),('fractions',[-.1]),('fractions',[1.1]),('fractions',[0.]*4097),('absolute_tolerance',-1),('absolute_tolerance',10**1000),('absolute_tolerance',float('inf')),('absolute_tolerance',True),('fractional_tolerance',0),('fractional_tolerance',1),('fractional_tolerance',float('nan')),('fresh',1),('extra',True)]:
            changed = copy.deepcopy(original)
            changed['diagnostic_length_requests'][0][key] = value
            with self.assertRaises(ValueError,msg=key):validate(changed)
        for count in (0,33):
            changed = copy.deepcopy(original)
            changed['diagnostic_length_requests'] *= count
            with self.assertRaises(ValueError):validate(changed)
        changed = copy.deepcopy(original)
        changed['diagnostics'] = False
        with self.assertRaises(ValueError):validate(changed)
        changed = copy.deepcopy(original)
        changed['diagnostic_length_requests'][0]['fractions'] = [0.]*4096
        changed['diagnostic_length_requests'] *= 8
        validate(changed)
        changed['diagnostic_length_requests'].append(changed['diagnostic_length_requests'][0])
        with self.assertRaises(ValueError):validate(changed)
