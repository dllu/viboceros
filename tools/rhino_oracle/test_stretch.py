"""Native Stretch source closure, protocol bounds and approximation evidence."""
import copy
import gzip
import hashlib
import json
import math
import tempfile
from pathlib import Path
from unittest import TestCase
from .generate_stretch import request as main
from .generate_stretch_fit import request as fit
from .generate_stretch_preferences import request as preferences
from .generate_stretch_details import request as details
from .structural_sdk import stage_helpers, validate_operations
HERE = Path(__file__).resolve().parent

def capture(stem):
    return json.loads((HERE/'observations'/(stem+'.json')).read_text())

class StretchTests(TestCase):
    def test_requests_producer_closure_and_complete_captures(self):
        digest = lambda b: hashlib.sha256(b).hexdigest()
        for stem,count,factory in [('stretch',142,main),('stretch_fit',18,fit),('stretch_preferences',4,preferences),('stretch_details',11,details)]:
            path = HERE/'fixtures'/(stem+'.json')
            request = json.loads(path.read_text())
            response = capture(stem)
            self.assertEqual(request,factory())
            validate_operations(request)
            self.assertEqual(len(response['results']),count)
            self.assertEqual([o['id'] for o in request['operations']],[o['id'] for o in response['results']])
            self.assertTrue(all('error' not in o for o in response['results']))
            provenance = response['provenance']
            self.assertEqual(digest(path.read_bytes()),provenance['request_sha256'])
            for name,expected in provenance['sources'].items():
                data = (HERE/name).read_bytes()
                if digest(data) != expected:
                    data = gzip.decompress((HERE/'observations/source_snapshots'/(expected+'.gz')).read_bytes())
                self.assertEqual(digest(data),expected,name)
            with tempfile.TemporaryDirectory() as folder:
                stage_helpers(request,HERE,Path(folder))
                self.assertTrue(list(Path(folder).iterdir()))
                for p in Path(folder).iterdir():
                    self.assertEqual(p.read_bytes(),(HERE/p.name).read_bytes())

    def test_point_map_and_invalid_axis_boundary_independently(self):
        count = 0
        for op,row in zip(main()['operations'],capture('stretch')['results']):
            if op['op'] != 'stretch_sdk':continue
            count += 1
            a,b = op['start'],op['end']
            length = math.dist(a,b)
            valid = length > 2.**-32
            self.assertEqual(row['value']['valid'],valid)
            axis = [(y-x)/length for x,y in zip(a,b)] if length else [0.]*3
            wanted = op['length'] if 'length' in op else sum((p-s)*n for p,s,n in zip(op['target'],a,axis))
            for source,actual in zip(op['points'],row['value']['points']):
                t = max(0.,min(1.,sum((p-s)*n for p,s,n in zip(source,a,axis))/length)) if valid else 0.
                delta = (max(wanted,0.)-length)*t*t*(3.-2.*t)
                expected = [p+n*delta for p,n in zip(source,axis)]
                self.assertLessEqual(math.dist(expected,actual),1e-11)
        self.assertEqual(count,23)

    def test_native_terminal_events_options_and_numeric_length_getter(self):
        for row in capture('stretch')['results']:
            if 'events' not in row['value']:continue
            value = row['value']
            names = [e['name'] for e in value['events']]
            self.assertIn('Stretch',names)
            expected = ['SelAll','Stretch'] if row['id'].startswith('postselected_') else ['Stretch','Cancel'] if row['id']=='identity_0' else ['Cancel','Stretch'] if row['id'] in ('identity_1','repeat_Cancel') else ['Stretch']
            self.assertEqual(names,expected)
            self.assertNotIn('Unknown command',value.get('history',''))
        rows = {r['id']:r['value'] for r in capture('stretch_preferences')['results']}
        for finish,factor in [('Cancel','1.25'),('Complete','0.75')]:
            history = rows['following_'+finish]['history']
            self.assertIn('Copy=Yes',history)
            self.assertIn('Rigid=No',history)
            self.assertIn('<'+factor+'>',history)

    def test_measured_fitting_floor_and_real_segments(self):
        rows = capture('stretch_fit')['results']
        for offset in (0,4,8,12):
            self.assertEqual(rows[offset+2]['value']['result'],rows[offset+3]['value']['result'])
        rows = capture('stretch_details')['results']
        for row in rows[:2]:
            v = row['value']
            self.assertTrue(v['success'])
            self.assertEqual(v['kind'],'PolyCurve')
            self.assertEqual([s['kind'] for s in v['segments']],['NurbsCurve','NurbsCurve'])
            self.assertEqual(v['segments'][0]['domain'][1],math.pi)
        physical = [m['maximum_forward_distance'] for row in rows[2:] for m in row['value']['measurements']]
        parametric = [m['maximum_same_parameter_error'] for row in rows[2:] for m in row['value']['measurements']]
        self.assertGreater(max(physical),3e-4)
        self.assertLess(max(physical),3.2e-4)
        self.assertLess(max(parametric),6.5e-4)
        self.assertTrue(all(m['samples'] in (513,2401) for row in rows[2:] for m in row['value']['measurements']))

    def test_invalid_recipes_are_rejected_before_launch(self):
        op = main()['operations'][0]
        for patch in [dict(length=float('nan')),dict(points=[]),dict(length=True),dict(target=[0.,0.,1.]),dict(extra=True)]:
            with self.subTest(patch=patch),self.assertRaises(ValueError):
                validate_operations(dict(operations=[dict(copy.deepcopy(op),**patch)]))
