"""The corner audit must retain strict images and explicit command limits."""
import copy
import json
import unittest
from .report_ribbon_corners import PROOF, ROOT, FAMILIES, read, report, summarize


class RibbonCornerRecordTests(unittest.TestCase):
    def inputs(self):
        r = json.loads((PROOF / 'receipts.json').read_bytes())
        corpus = {f: (json.loads((ROOT / ('tools/rhino_oracle/fixtures/'+f+'.json')).read_bytes()),
            json.loads(read(ROOT / ('tools/rhino_oracle/observations/'+f+'.json.gz')))) for f in FAMILIES}
        return corpus, read(ROOT / r['checks']['portable']['log']).decode()

    def test_frozen_record_is_reproducible(self):
        self.assertEqual(report(), json.loads((ROOT / 'docs/ribbon-corner-validation.json').read_bytes()))

    def test_missing_case_and_relaxed_bound_are_rejected(self):
        c, log = self.inputs()
        with self.assertRaises(AssertionError): summarize(c, log.replace('RIBBON_CORNERS ', 'MISSING ', 1))
        lines=[]; changed=False
        for line in log.splitlines():
            if not changed and 'RIBBON_CORNERS ' in line:
                prefix, text = line.split('RIBBON_CORNERS ',1); row=json.loads(text)
                row['incidence_bound']=1e-5;line=prefix+'RIBBON_CORNERS '+json.dumps(row);changed=True
            lines.append(line)
        with self.assertRaises(AssertionError): summarize(c, '\n'.join(lines))

    def test_unknown_native_command_and_false_command_parity_are_rejected(self):
        c, log = self.inputs(); altered=copy.deepcopy(c)
        altered['ribbon_polyline'][1]['results'][0]['value']['history']+='\nUnknown command: _Yes'
        with self.assertRaises(AssertionError): summarize(altered,log)
        with self.assertRaises(AssertionError): summarize(c,log.replace('"full_ribbon_command":false','"full_ribbon_command":true',1))


if __name__ == '__main__': unittest.main()
