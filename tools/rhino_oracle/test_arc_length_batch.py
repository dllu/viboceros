import copy,json,math
from pathlib import Path
from unittest import TestCase
from .arc_length_batch_probe import validate
from .arc_length_batch_comparison import comparison
BASE=Path(__file__).parent


class ArcLengthBatchTests(TestCase):
    def request(self):return json.loads((BASE/'fixtures/arc_length_batch.json').read_text())
    def read(self,name):return json.loads((BASE/'observations'/('arc_length_batch_'+name+'_20261016.json')).read_text())

    def test_bounded_fractions_curve_definitions_and_empty_batches(self):
        q=self.request();self.assertEqual(len(q['operations']),14)
        for op in q['operations']:validate(op)
        original=q['operations'][0]
        for key,value in [('id','bad\nPICK'),('fractions',[True]),('fractions',[-.1]),('fractions',[1.1]),('fractions',[10**1000]),('fractions',[float('nan')]),('fractions',[float('inf')]),('fractions',[0]*4097),('curve',dict(degree=True,control_points=[],knots=[]))]:
            changed=copy.deepcopy(original);changed[key]=value
            with self.assertRaises(ValueError,msg=key):validate(changed)
        changed=copy.deepcopy(original);changed['fractions']=[.5]*4096;validate(changed)

    def test_native_rational_line_and_circle_positions_have_analytic_expectations(self):
        q=self.request();n=self.read('rhino');ops={o['id']:o for o in q['operations']}
        for r in n['results']:
            name=r['id'];v=r['value'];f=ops[name]['fractions']
            if name=='rational-line':
                for fraction,t,p in zip(f,v['parameters'],v['points']):
                    self.assertAlmostEqual(t,10+30*fraction/(7-4*fraction),places=9)
                    self.assertLess(math.dist(p,[10*fraction,0,0]),1e-9)
            elif name in ('quarter-circle','closed-circle'):
                angle=math.pi/2 if name=='quarter-circle'else 2*math.pi
                for fraction,p in zip(f,v['points']):self.assertLess(math.dist(p,[math.cos(angle*fraction),math.sin(angle*fraction),0]),1e-9)

    def test_native_comparison_covers_parameters_points_order_duplicates_and_endpoints(self):
        q=self.request();local,native=self.read('viboceros'),self.read('rhino');r=comparison(q,local,native)
        self.assertEqual(r,self.read('comparison'));self.assertEqual(r['matched_cases'],13);self.assertEqual(r['geometry_matched_cases'],14)
        self.assertEqual(sum(v['requests']for v in r['results']),1815)
        for field in ('point','parameter','length','domain'):
            changed=copy.deepcopy(native);v=changed['results'][0]['value']
            if field=='point':v['points'][0][0]+=.01
            elif field=='parameter':v['parameters'][0]-=.01
            elif field=='length':v['length']+=.01
            else:v['domain'][0]-=.01
            bad=comparison(q,local,changed)
            self.assertEqual(bad['matched_cases'],12,field);self.assertEqual(bad['geometry_matched_cases'],13,field)

    def test_stationary_endpoint_keeps_the_analytic_inverse_and_exposes_native_parameter_error(self):
        q=self.request();local,native=self.read('viboceros'),self.read('rhino')
        i=next(i for i,o in enumerate(q['operations'])if o['id']=='stationary-end');op=q['operations'][i]
        self.assertEqual([c['point']for c in op['curve']['control_points']],[[0,0,0],[0,0,0],[1,0,0]])
        actual=local['results'][i]['value'];reference=native['results'][i]['value']
        for f,t,p in zip(op['fractions'],actual['parameters'],actual['points']):
            self.assertLess(abs(t-math.sqrt(f)),1e-8)
            self.assertLess(math.dist(p,[f,0,0]),1e-12)
        self.assertGreater(max(abs(t-math.sqrt(f))for f,t in zip(op['fractions'],reference['parameters'])),1e-5)
        row=comparison(q,local,native)['results'][i]
        self.assertFalse(row['within_1e_minus_8']);self.assertTrue(row['geometry_within_document_tolerance'])

    def test_incomplete_or_nonfinite_native_results_cannot_pass(self):
        q=self.request();local,native=self.read('viboceros'),self.read('rhino')
        for field in ('nonfinite','shape','identity'):
            changed=copy.deepcopy(native);v=changed['results'][0]
            if field=='nonfinite':v['value']['points'][0][0]=float('nan')
            elif field=='shape':v['value']['parameters'].pop()
            else:v['id']='missing'
            with self.assertRaises(ValueError,msg=field):comparison(q,local,changed)
