"""Independent plane/cylinder geometry and retained kernel-repair evidence."""
import json
import math
from pathlib import Path
import unittest
from tools.verify_provenance import verify
ROOT=Path(__file__).resolve().parents[2]


class SurfacePullTests(unittest.TestCase):
    def test_direct_repair_recovers_only_the_three_off_surface_crossings(self):
        read=lambda name:[json.loads(line)for line in (ROOT/('docs/smlib-surface-pull-'+name+'.jsonl')).read_text().splitlines()]
        baseline,repaired=read('baseline'),read('repaired')
        self.assertEqual(len(baseline),12);self.assertEqual(len(repaired),12)
        for a,b in zip(baseline,repaired):
            self.assertEqual(a['status'],1000);self.assertEqual(b['status'],1000)
            changed=b['height']==1 and b['case'] in (1,2,3)
            if changed:self.assertEqual(a['curve_count'],0)
            else:self.assertEqual(a,b)
            expected=[(1.,9.),(0.,10.),(1.,10.),(0.,9.),None,None][b['case']]
            self.assertEqual(b['curve_count'],int(expected is not None))
            if expected is None:continue
            points=b['curves'][0]
            self.assertLessEqual(abs(min(p[0]for p in points)-expected[0]),1e-9)
            self.assertLessEqual(abs(max(p[0]for p in points)-expected[1]),1e-9)
            for p in points:self.assertLessEqual(abs(p[1]-5.)+abs(p[2]),1e-9)

    def test_retained_native_outputs_match_analytic_planes_and_cylinders(self):
        request=json.loads((ROOT/'tools/rhino_oracle/fixtures/pull_curve_surface.json').read_text())
        rows=json.loads((ROOT/'tools/rhino_oracle/observations/pull_curve_surface_viboceros_native.json').read_text())['results']
        self.assertEqual([r['id']for r in rows],[op['id']for op in request['operations']])
        tolerance=request['tolerance']['absolute']
        for row in rows:
            id=row['id'];curves=row['value']['curves']
            if id=='plane-miss':self.assertEqual(curves,[]);continue
            self.assertTrue(curves)
            expected={'plane-inside':8.,'plane-cross':10.,'plane-translated-domains':16.,'cylinder-axial':8.,'cylinder-loop':4.*math.pi}[id]
            self.assertLessEqual(abs(sum(c['length']for c in curves)-expected),tolerance)
            if id=='cylinder-axial':self.assertEqual(len(curves),2)
            for curve in curves:
                for p in curve['samples']:
                    if id.startswith('cylinder'):
                        self.assertLessEqual(abs(math.hypot(p[0],p[1])-2.),tolerance)
                        if id=='cylinder-loop':self.assertLessEqual(abs(p[2]-2.),tolerance)
                        else:self.assertTrue(.5-tolerance<=p[2]<=4.5+tolerance)
                    elif id=='plane-translated-domains':
                        self.assertLessEqual(abs(p[2]-3e6)+abs(p[1]+2e6-10.),tolerance)
                    else:self.assertLessEqual(abs(p[2])+abs(p[1]-5.),tolerance)

    def test_qualification_preserves_sources_and_unqualified_scope(self):
        q=json.loads((ROOT/'docs/smlib-surface-pull-qualification.json').read_text())
        self.assertFalse(q['pull_command_implemented'])
        self.assertFalse(q['full_rhino_parity'])
        self.assertFalse(q['performance_qualified'])
        self.assertEqual(q['fresh_rhino_observations'],0)
        self.assertGreaterEqual(verify(q),15)
