"""Guard the distinction between native-basis, source, and timing evidence."""
import copy
import json
import unittest
from . import report_surface_fit_sampling as record


def replace(log, prefix, rows):
    it = iter(rows)
    return '\n'.join(prefix+' '+json.dumps(next(it)) if s.startswith(prefix+' ') else s
                     for s in log.splitlines())


class SamplingRecordTests(unittest.TestCase):
    def test_saved_record_reproduces(self):
        path = record.ROOT/'docs/surface-fit-sampling-validation.json'
        self.assertEqual(record.report(),json.loads(path.read_bytes()))

    def test_source_tolerance_cannot_replace_native_gate_or_claim_full_policy(self):
        q = json.loads((record.ROOT/'tools/rhino_oracle/fixtures/surface_fit_covariance.json').read_bytes())
        n = json.loads(record.read(record.ROOT/'tools/rhino_oracle/observations/surface_fit_covariance.json.gz'))
        log = record.read(record.PROOF/'portable.log.gz').decode()
        original = record.rows(log,'FIT_COVARIANCE')
        for key,value in [('native_bound',1e-8),('full_native_policy',True),('rechecked_bound',None)]:
            changed = copy.deepcopy(original); changed[0][key] = value
            with self.assertRaises(AssertionError):
                record.summarize(q,n,replace(log,'FIT_COVARIANCE',changed))

    def test_timing_rows_require_identical_control_fingerprints_and_honest_scope(self):
        before = record.read(record.PROOF/'sampling_before.log.gz').decode()
        after = record.read(record.PROOF/'sampling_after.log.gz').decode()
        original = record.rows(after,'FIT_SAMPLING')
        for key,value in [('fingerprint','changed'),('source_accuracy_certified',True),('median_seconds',0.)]:
            changed = copy.deepcopy(original); changed[0][key] = value
            with self.assertRaises(AssertionError):
                record.timings(before,replace(after,'FIT_SAMPLING',changed))


if __name__ == '__main__':
    unittest.main()
