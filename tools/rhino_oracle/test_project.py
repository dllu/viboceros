"""Independent image diagnostics for retained directional projection outputs."""
import json
import math
from pathlib import Path
from unittest import TestCase
from tools.verify_provenance import verify
ROOT=Path(__file__).resolve().parents[2]
def read(path): return json.loads((ROOT/path).read_text())

class ProjectQualificationTests(TestCase):
    def test_independent_finite_planes_and_curved_graph_images(self):
        q=read('tools/rhino_oracle/fixtures/project_curve_surface.json')
        response=read('tools/rhino_oracle/observations/project_curve_surface_viboceros_native.json')
        self.assertEqual(response['engine'],'viboceros')
        self.assertEqual([r['id'] for r in response['results']],[r['id'] for r in q['operations']])
        for row in response['results']:
            curves=row['value']['curves']
            self.assertEqual(len(curves),int(row['id']!='project_miss'))
            for c in curves:
                self.assertFalse(c['closed']); self.assertEqual(len(c['samples']),33)
                self.assertTrue(math.isfinite(c['length'])); self.assertGreater(c['length'],0.)
                for x,y,z in c['samples']:
                    self.assertTrue(all(math.isfinite(v)for v in (x,y,z)))
                    self.assertTrue(-1e-6<=x<=10.+1e-6); self.assertTrue(-1e-6<=y<=10.+1e-6)
                    self.assertLess(abs(z-(0.04*x*y if row['id'].endswith('graph') else 0.)),q['tolerance']['absolute'])
                    if row['id']=='project_curved_graph': self.assertLess(abs(y-2.-0.75*(x-1.)),1e-6)
                    if row['id']=='project_oblique': self.assertLess(abs(y-2.-1.2*(x-5.5)),1e-6)
        rows={r['id']:r['value']['curves']for r in response['results']}
        self.assertAlmostEqual(rows['project_plane'][0]['length'],8.,places=7)
        self.assertAlmostEqual(rows['project_clipped_plane'][0]['length'],10.,places=7)

    def test_rational_source_direction_uses_independent_bezier_formula(self):
        row=next(r for r in read('tools/rhino_oracle/observations/project_curve_surface_viboceros_native.json')['results']if r['id']=='project_rational_graph')
        def xy(t):
            a,b,c=(1.-t)**2,1.5*t*(1.-t),t*t
            return ((a+5.*b+9.*c)/(a+b+c),(a+9.*b+c)/(a+b+c))
        for x,y,_ in row['value']['curves'][0]['samples']:
            low,high=0.,1.
            for _ in range(64):
                t=(low+high)/2.
                if xy(t)[0]<x: low=t
                else: high=t
            self.assertLess(abs(y-xy((low+high)/2.)[1]),1e-6)

    def test_qualification_is_local_and_preserves_source_hashes(self):
        p=read('docs/project-qualification.json')
        self.assertFalse(p['full_rhino_parity']); self.assertFalse(p['performance_qualified'])
        self.assertEqual(p['rhino_attempt']['status'],'startup_timeout')
        self.assertFalse(p['rhino_attempt']['worker_response'])
        self.assertGreaterEqual(verify(p),10)
