"""Retained measurements must describe the generated workloads and geometry."""
import hashlib
import json
import math
from pathlib import Path
import statistics
import unittest
from tools.rhino_oracle.mesh_pull_performance import request
from tools.verify_provenance import verify

ROOT=Path(__file__).resolve().parents[2]


def read(path):
    return json.loads((ROOT/path).read_text())


class MeshPullPerformanceTests(unittest.TestCase):
    def test_request_timings_and_all_raw_outcomes_are_retained(self):
        payload=json.dumps(request(),sort_keys=True,separators=(',',':')).encode()
        for revision in ('before','after'):
            report=read('tools/rhino_oracle/observations/mesh_pull_performance_'+revision+'.json')
            self.assertEqual(report['request_sha256'],hashlib.sha256(payload).hexdigest())
            self.assertEqual(len(report['runs']),report['repeats'])
            self.assertEqual(report['repeats'],3)
            for operation in request()['operations']:
                identifier=operation['id']
                outcomes=[next(o for o in run['outcomes'] if o.get('id',o.get('result',{}).get('id'))==identifier)for run in report['runs']]
                summary=report['summary'][identifier]
                self.assertEqual(summary['statuses'],[o['status']for o in outcomes])
                times=[o['result']['elapsed_ns']for o in outcomes if o['status']=='success']
                self.assertEqual(summary['elapsed_ns'],times)
                self.assertEqual(summary['median_elapsed_ns'],statistics.median(times)if times else None)
                expected='failure'if revision=='before'and identifier=='line-grid-24'else'success'
                self.assertEqual(summary['statuses'],[expected]*3)

    def test_successful_curves_match_their_analytic_loci(self):
        tolerance=request()['tolerance']['absolute']
        for revision in ('before','after'):
            for run in read('tools/rhino_oracle/observations/mesh_pull_performance_'+revision+'.json')['runs']:
                for outcome in run['outcomes']:
                    if outcome['status']!='success':
                        self.assertIn('mesh pull exceeded',outcome['error']['message'])
                        continue
                    result=outcome['result'];identifier=result['id']
                    curves=result['value']['curves'];self.assertEqual(len(curves),1)
                    curve=curves[0];circle=identifier=='circle-two-faces'
                    self.assertEqual(curve['closed'],circle)
                    self.assertLessEqual(abs(curve['length']-(4.*math.pi if circle else 7.)),tolerance)
                    for x,y,z in curve['samples']:
                        self.assertLessEqual(abs(z),tolerance)
                        if circle:self.assertLessEqual(abs(math.hypot(x,y)-2.),tolerance)
                        else:
                            self.assertLessEqual(abs(y-.25),tolerance)
                            self.assertTrue(-3.5-tolerance<=x<=3.5+tolerance)

    def test_qualification_keeps_rhino_and_broader_speed_limits_explicit(self):
        record=read('docs/mesh-pull-performance.json')
        self.assertFalse(record['full_rhino_parity'])
        self.assertFalse(record['performance_qualified'])
        self.assertEqual(record['fresh_rhino_observations'],0)
        self.assertEqual(record['baseline_revision'],'151fd7d5f9dfa7eeb836466e015c06abc4af067e')
        for field,revision in [('baseline_binary_sha256','before'),('after_binary_sha256','after')]:
            self.assertEqual(record[field],read('tools/rhino_oracle/observations/mesh_pull_performance_'+revision+'.json')['binary_sha256'])
        self.assertGreaterEqual(verify(record),10)
