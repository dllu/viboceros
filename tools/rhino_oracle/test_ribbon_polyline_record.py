"""Prevent replay logs from promoting unproved images or topology to parity."""
import copy
import json
import unittest
from .report_ribbon_polyline import PROOF, ROOT, read, report, summarize


class PolylineRibbonRecordTests(unittest.TestCase):
    def inputs(self):
        receipt = json.loads((PROOF / 'receipts.json').read_bytes())
        return (json.loads((ROOT / 'tools/rhino_oracle/fixtures/ribbon_polyline.json').read_bytes()),
            json.loads(read(ROOT / 'tools/rhino_oracle/observations/ribbon_polyline.json.gz')),
            read(ROOT / receipt['checks']['portable']['log']).decode())

    def test_frozen_record_remains_reproducible(self):
        self.assertEqual(report(), json.loads((ROOT / 'docs/ribbon-polyline-validation.json').read_bytes()))

    def test_a_missing_case_or_relaxed_image_bound_is_rejected(self):
        q, n, log = self.inputs()
        with self.assertRaises(AssertionError): summarize(q, n, log.replace('RIBBON_POLYLINE ', 'MISSING ', 1))
        rows = [json.loads(s.split(' ', 1)[1]) for s in log.splitlines() if s.startswith('RIBBON_POLYLINE ')]
        rows[0]['support_coverage_bound'] = 1e-5
        altered = log + '\n'
        index = 0
        lines = []
        for line in altered.splitlines():
            if line.startswith('RIBBON_POLYLINE '):
                line = 'RIBBON_POLYLINE ' + json.dumps(rows[index]); index += 1
            lines.append(line)
        with self.assertRaises(AssertionError): summarize(q, n, '\n'.join(lines))

    def test_native_history_and_known_point_gaps_cannot_be_silently_qualified(self):
        q, n, log = self.inputs()
        altered = copy.deepcopy(n); altered['results'][0]['value']['history'] += '\nUnknown command: _Yes'
        with self.assertRaises(AssertionError): summarize(q, altered, log)
        lines = []
        for line in log.splitlines():
            if line.startswith('RIBBON_POLYLINE '):
                row = json.loads(line.split(' ', 1)[1])
                if row['id'] == 'right-none': row['incidence_bound'] = 0.
                line = 'RIBBON_POLYLINE ' + json.dumps(row)
            lines.append(line)
        with self.assertRaises(AssertionError): summarize(q, n, '\n'.join(lines))


if __name__ == '__main__': unittest.main()
