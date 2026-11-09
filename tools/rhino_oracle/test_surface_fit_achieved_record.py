"""Keep SDK reports, complete output proofs and weak witnesses distinct."""
import copy
import json
import unittest
from . import report_surface_fit_achieved as record

class AchievedRecordTests(unittest.TestCase):
    def test_record_reproduces(self):
        self.assertEqual(record.report(),json.loads((record.ROOT/'docs/surface-fit-achieved-validation.json').read_bytes()))

    def test_false_certification_and_relaxed_native_gate_are_rejected(self):
        q=json.loads((record.ROOT/'tools/rhino_oracle/fixtures/surface_fit_achieved.json').read_bytes())
        n=json.loads(record.read(record.ROOT/'tools/rhino_oracle/observations/surface_fit_achieved.json.gz'))
        log=record.read(record.PROOF/'portable.log.gz').decode();rows=record.rows(log,'FIT_ACHIEVED')
        def replace(changed):
            it=iter(changed)
            return '\n'.join('FIT_ACHIEVED '+json.dumps(next(it))if s.startswith('FIT_ACHIEVED ')else s for s in log.splitlines())
        changed=copy.deepcopy(rows);changed[0]['sampled_certified']=True
        with self.assertRaises(AssertionError):record.summarize(q,n,replace(changed))
        changed=copy.deepcopy(rows);next(r for r in changed if r['native_bound']is not None)['native_bound']=1e-8
        with self.assertRaises(AssertionError):record.summarize(q,n,replace(changed))

    def test_zero_null_report_never_becomes_a_geometry_bound(self):
        q=json.loads((record.ROOT/'tools/rhino_oracle/fixtures/surface_fit_achieved.json').read_bytes())
        n=json.loads(record.read(record.ROOT/'tools/rhino_oracle/observations/surface_fit_achieved.json.gz'))
        log=record.read(record.PROOF/'portable.log.gz').decode();changed=copy.deepcopy(n)
        next(r for r in changed['results']if r['value']['surface']is None)['value']['achieved_tolerance']=1e-5
        with self.assertRaises(AssertionError):record.summarize(q,changed,log)

if __name__=='__main__':unittest.main()
