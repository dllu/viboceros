"""Independent diagnostics for directional mesh projection; no expected solver curves."""
import json
import math
from pathlib import Path
from unittest import TestCase
from tools.verify_provenance import verify
ROOT=Path(__file__).resolve().parents[2]
def read(path):return json.loads((ROOT/path).read_text())
def records(mode):return read('tools/rhino_oracle/observations/project_mesh_viboceros_'+mode+'.json')['results']

class MeshProjectQualificationTests(TestCase):
    def test_both_backends_keep_expected_images_and_contact_counts(self):
        fixture=read('tools/rhino_oracle/fixtures/project_mesh.json')
        for mode in ('native','portable'):
            rows=records(mode);self.assertEqual([r['id']for r in rows],[o['id']for o in fixture['operations']])
            for row in rows:
                v=row['value'];identifier=row['id']
                if 'points'in v:
                    count=0 if identifier.endswith('miss') else 2 if identifier.endswith('two_sheets') else 1
                    self.assertEqual(len(v['points']),count)
                    if identifier.endswith('warped'):self.assertEqual(v['points'],[[0.25,0.75,0.]])
                    if identifier.endswith('two_sheets'):self.assertEqual(v['points'],[[0.25,0.25,0.],[0.25,0.25,2.]])
                else:
                    self.assertEqual(len(v['curves']),int(not identifier.endswith(('miss','vertex_touch'))),identifier)
                    for c in v['curves']:
                        self.assertEqual(len(c['samples']),33);self.assertGreater(c['length'],0.);self.assertFalse(c['closed'])
                        for x,y,z in c['samples']:
                            self.assertTrue(all(math.isfinite(t)for t in (x,y,z)));self.assertTrue(-1e-6<=x<=1.+1e-6);self.assertTrue(-1e-6<=y<=1.+1e-6)
                            expected=max(x+y-1.,0.) if identifier.endswith('warped') else 0.
                            self.assertLess(abs(z-expected),1e-6)
                        if identifier.endswith('line_warped'):self.assertAlmostEqual(c['length'],0.75+0.25*math.sqrt(2.),places=6)

    def test_retained_payloads_agree_without_comparing_timings(self):
        a,b=records('native'),records('portable')
        def compare(a,b):
            if isinstance(a,dict):
                self.assertEqual(set(a),set(b))
                for key in a:compare(a[key],b[key])
            elif isinstance(a,list):
                self.assertEqual(len(a),len(b))
                for x,y in zip(a,b):compare(x,y)
            elif isinstance(a,float):self.assertAlmostEqual(a,b,delta=1e-6)
            else:self.assertEqual(a,b)
        for x,y in zip(a,b):self.assertEqual(x['id'],y['id']);compare(x['value'],y['value'])

    def test_qualification_preserves_source_bytes_and_startup_failure(self):
        p=read('docs/project-mesh-qualification.json')
        self.assertFalse(p['full_rhino_parity']);self.assertFalse(p['performance_qualified']);self.assertEqual(p['recipes'],13)
        self.assertEqual(p['rhino_attempt']['status'],'startup_timeout');self.assertFalse(p['rhino_attempt']['worker_response'])
        self.assertGreaterEqual(verify(p),15)
