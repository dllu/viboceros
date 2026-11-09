"""Reject missing cases, concealed support gaps and relaxed fixed-plane gates."""
import copy
import json
import unittest
from .report_ribbon_cplane import PROOF, ROOT, read, corpus, summarize, report


class RibbonCPlaneRecordTests(unittest.TestCase):
    def inputs(self):
        receipt = json.loads((PROOF / 'receipts.json').read_bytes())
        return corpus(), read(ROOT / receipt['checks']['portable']['log']).decode()

    def test_frozen_record_is_reproducible(self):
        self.assertEqual(report(), json.loads((ROOT / 'docs/ribbon-cplane-validation.json').read_bytes()))

    def test_missing_rows_relaxed_bounds_and_false_support_are_rejected(self):
        inputs, log = self.inputs()
        with self.assertRaises(AssertionError):
            summarize(inputs, log.replace('RIBBON_COMMAND ', 'MISSING ', 1))
        for key, value, supported in [('image_bound', 1e-5, True), ('full_command_parity', True, True),
                                      ('local_supported', True, False), ('failure_atomic', False, False)]:
            lines = []
            changed = False
            for line in log.splitlines():
                if not changed and 'RIBBON_COMMAND ' in line:
                    prefix, text = line.split('RIBBON_COMMAND ', 1)
                    row = json.loads(text)
                    if row['local_supported'] is supported:
                        row[key] = value
                        line = prefix + 'RIBBON_COMMAND ' + json.dumps(row)
                        changed = True
                lines.append(line)
            self.assertTrue(changed)
            with self.assertRaises(AssertionError):
                summarize(inputs, '\n'.join(lines))

    def test_unknown_native_command_and_false_u_correspondence_are_rejected(self):
        inputs, log = self.inputs()
        bad = copy.deepcopy(inputs)
        bad['ribbon_cplane'][1]['results'][0]['value']['history'] += '\nUnknown command: _Yes'
        with self.assertRaises(AssertionError):
            summarize(bad, log)
        lines = []
        for line in log.splitlines():
            if 'RIBBON_COMMAND ' in line:
                prefix, text = line.split('RIBBON_COMMAND ', 1)
                row = json.loads(text)
                if row['family'] == 'ribbon_cplane' and row['local_supported']:
                    row['u_correspondence_compared'] = True
                    line = prefix + 'RIBBON_COMMAND ' + json.dumps(row)
            lines.append(line)
        with self.assertRaises(AssertionError):
            summarize(inputs, '\n'.join(lines))


if __name__ == '__main__':
    unittest.main()
