"""Exact rational image checks for retained infinite-plane curve definitions."""
import json
from fractions import Fraction as F
from pathlib import Path
from unittest import TestCase
from tools.verify_provenance import verify
ROOT=Path(__file__).resolve().parents[2]

def read(path):return json.loads((ROOT/path).read_text())

class ProjectPlaneTests(TestCase):
    def test_backend_control_images_match_independent_rational_projection(self):
        q=read('tools/rhino_oracle/fixtures/project_curve_plane.json')
        for backend in ('portable','native'):
            rows=read('tools/rhino_oracle/observations/project_curve_plane_viboceros_'+backend+'.json')['results']
            self.assertEqual([r['id'] for r in rows],[o['id'] for o in q['operations']])
            for op,row in zip(q['operations'],rows):
                source=op['curve'];curves=row['value']['curves'];self.assertEqual(len(curves),1)
                actual=curves[0]
                self.assertEqual(actual['degree'],source['degree']);self.assertEqual(actual['knots'],source['knots'])
                self.assertEqual(actual['domain'],[source['knots'][source['degree']],source['knots'][len(source['control_points'])]])
                n,d,o=([F(x) for x in op[k]] for k in ('normal','direction','origin'))
                denominator=sum(a*b for a,b in zip(n,d))
                self.assertEqual(len(source['control_points']),len(actual['control_points']))
                for a,b in zip(source['control_points'],actual['control_points']):
                    self.assertEqual(a['weight'],b['weight'])
                    p=[F(x) for x in a['point']]
                    distance=sum(normal*(value-origin) for normal,value,origin in zip(n,p,o))/denominator
                    expected=[float(value-distance*direction) for value,direction in zip(p,d)]
                    self.assertLessEqual(sum((x-y)**2 for x,y in zip(expected,b['point'])),q['tolerance']['absolute']**2)

    def test_qualification_and_source_records_keep_capture_limits_explicit(self):
        q=read('docs/project-infinite-plane-qualification.json')
        self.assertFalse(q['full_rhino_parity']);self.assertFalse(q['performance_qualified']);self.assertEqual(q['fresh_rhino_observations'],0)
        self.assertGreaterEqual(verify(q),15)
