"""Keep smooth Ribbon evidence strict, complete and separate from command parity."""
import copy
import json
import unittest
from .report_ribbon_smooth import PROOF, ROOT, FAMILIES, read, report, summarize


class RibbonSmoothRecordTests(unittest.TestCase):
    def inputs(self):
        r = json.loads((PROOF / 'receipts.json').read_bytes())
        corpus = {f: (json.loads((ROOT / ('tools/rhino_oracle/fixtures/'+f+'.json')).read_bytes()),
            json.loads(read(ROOT / ('tools/rhino_oracle/observations/'+f+'.json.gz')))) for f in FAMILIES}
        return corpus, read(ROOT / r['checks']['portable']['log']).decode()

    def test_frozen_record_is_reproducible(self):
        self.assertEqual(report(), json.loads((ROOT / 'docs/ribbon-smooth-validation.json').read_bytes()))

    def test_missing_case_relaxed_bound_and_false_command_parity_are_rejected(self):
        c, log = self.inputs()
        with self.assertRaises(AssertionError): summarize(c, log.replace('RIBBON_SMOOTH ', 'MISSING ', 1))
        for key, value in [('incidence_bound', 1e-5), ('full_ribbon_command', True), ('distance_certified', True)]:
            lines = []; changed = False
            for line in log.splitlines():
                if not changed and 'RIBBON_SMOOTH ' in line:
                    prefix, text = line.split('RIBBON_SMOOTH ', 1); row = json.loads(text)
                    row[key] = value; line = prefix+'RIBBON_SMOOTH '+json.dumps(row); changed = True
                lines.append(line)
            with self.assertRaises(AssertionError): summarize(c, '\n'.join(lines))

    def test_unknown_native_command_is_rejected(self):
        c, log = self.inputs(); altered = copy.deepcopy(c)
        altered['ribbon_smooth'][1]['results'][0]['value']['history'] += '\nUnknown command: _Yes'
        with self.assertRaises(AssertionError): summarize(altered, log)


if __name__ == '__main__': unittest.main()
