"""Analytic command outputs, retained source hashes and explicit parity limits."""
import json
import math
from pathlib import Path
import unittest
from tools.verify_provenance import verify
ROOT=Path(__file__).resolve().parents[2]


def read(path):return json.loads((ROOT/path).read_text())


class PullCommandTests(unittest.TestCase):
    def test_native_and_portable_command_outputs_match_their_analytic_images(self):
        for backend in ('native','portable'):
            name='pull_curve_surface_command'+('_portable'if backend=='portable'else'')
            request=read('tools/rhino_oracle/fixtures/'+name+'.json')
            rows=read('tools/rhino_oracle/observations/pull_curve_surface_command_viboceros_'+backend+'.json')['results']
            self.assertEqual([r['id']for r in rows],[op['id']for op in request['operations']])
            tolerance=request['tolerance']['absolute']
            for row in rows:
                with self.subTest(backend=backend,id=row['id']):
                    id=row['id'];curves=row['value']['curves']
                    expected={'plane-inside':8.,'plane-cross':10.,'plane-miss':0.,'plane-translated-domains':16.,'cylinder-axial':4.,'cylinder-loop':4.*math.pi}[id]
                    self.assertLessEqual(abs(sum(c['length']for c in curves)-expected),tolerance)
                    if id=='plane-miss':self.assertEqual(curves,[]);continue
                    if id!='cylinder-loop':self.assertEqual(len(curves),1)
                    for curve in curves:
                        for p in curve['samples']:
                            if id.startswith('cylinder'):
                                self.assertLessEqual(abs(math.hypot(p[0],p[1])-2.),tolerance)
                                if id=='cylinder-axial':self.assertTrue(.5-tolerance<=p[2]<=4.5+tolerance)
                                else:self.assertLessEqual(abs(p[2]-2.),tolerance)
                            else:
                                origin=[1e6,-2e6,3e6]if id=='plane-translated-domains'else[0,0,0]
                                scale=2. if id=='plane-translated-domains'else 1.
                                self.assertLessEqual(abs(p[1]-origin[1]-scale*5.)+abs(p[2]-origin[2]),tolerance)
                                lo,hi=(0.,10.)if id=='plane-cross'else(1.,9.)
                                self.assertTrue(origin[0]+scale*lo-tolerance<=p[0]<=origin[0]+scale*hi+tolerance)

    def test_command_qualification_retains_sources_without_claiming_native_parity(self):
        q=read('docs/pull-command-qualification.json')
        self.assertTrue(q['pull_command_implemented'])
        self.assertFalse(q['full_rhino_parity'])
        self.assertFalse(q['performance_qualified'])
        self.assertEqual(q['fresh_rhino_observations'],0)
        self.assertGreaterEqual(verify(q),20)
