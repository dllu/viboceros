"""Independent geometric checks for retained IPlane backend observations."""
import json
from pathlib import Path
import unittest
from tools.verify_provenance import verify
ROOT=Path(__file__).resolve().parents[2]


def read(path):return json.loads((ROOT/path).read_text())


class IPlaneObservationTests(unittest.TestCase):
    def test_retained_outputs_match_analytic_crossings_lengths_and_closedness(self):
        request=read('tools/rhino_oracle/fixtures/iplane.json')
        tolerance=request['tolerance']['absolute']
        for backend in ('portable','native'):
            rows=read('tools/rhino_oracle/observations/iplane_viboceros_'+backend+'.json')['results']
            self.assertEqual([r['id'] for r in rows],[op['id'] for op in request['operations']])
            values={r['id']:r['value'] for r in rows}
            self.assertEqual(values['line-cross'],dict(points=[[10.,20.,0.]],curves=[]))
            self.assertEqual(values['line-miss'],dict(points=[],curves=[]))
            for id,length,closed in [('line-contained',2.,False),('surface-cross',3.,False),('mesh-cross',3.,False),('box-cross',8.,True)]:
                with self.subTest(backend=backend,id=id):
                    value=values[id]
                    self.assertEqual(value['points'],[]);self.assertEqual(len(value['curves']),1)
                    curve=value['curves'][0]
                    self.assertLessEqual(abs(curve['length']-length),tolerance)
                    self.assertEqual(curve['closed'],closed)
                    self.assertEqual(len(curve['samples']),32 if closed else 33)
                    for p in curve['samples']:
                        self.assertLessEqual(abs(p[2]),tolerance)
                        if id in ('surface-cross','mesh-cross'):
                            self.assertLessEqual(abs(p[0]-2.),tolerance)
                            self.assertTrue(-tolerance<=p[1]<=3.+tolerance)
                        if id=='box-cross':
                            self.assertLessEqual(abs(max(abs(p[0]),abs(p[1]))-1.),tolerance)
                    endpoints=curve['samples'][0],curve['samples'][-1]
                    if id=='line-contained':self.assertEqual(endpoints,([0.,0.,0.],[2.,0.,0.]))
                    if id in ('surface-cross','mesh-cross'):
                        for actual,expected in zip(endpoints,([2.,0.,0.],[2.,3.,0.])):
                            self.assertLessEqual(sum((a-b)**2 for a,b in zip(actual,expected)),tolerance**2)

    def test_qualification_retains_sources_and_explicit_capture_limits(self):
        record=read('docs/iplane-qualification.json')
        self.assertFalse(record['full_rhino_parity'])
        self.assertFalse(record['performance_qualified'])
        self.assertEqual(record['fresh_rhino_observations'],0)
        self.assertGreaterEqual(verify(record),15)
