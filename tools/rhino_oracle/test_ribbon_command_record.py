"""No omitted native-success gaps or relaxed Ribbon command image checks."""
import copy
import json
import unittest
from .report_ribbon_command import PROOF,ROOT,FAMILIES,read,report,summarize


class RibbonCommandRecordTests(unittest.TestCase):
    def inputs(self):
        r=json.loads((PROOF/'receipts.json').read_bytes())
        corpus={f:(json.loads((ROOT/('tools/rhino_oracle/fixtures/'+f+'.json')).read_bytes()),
            json.loads(read(ROOT/('tools/rhino_oracle/observations/'+f+'.json.gz'))))for f in FAMILIES}
        return corpus,read(ROOT/r['checks']['portable']['log']).decode()

    def test_frozen_record_is_reproducible(self):
        self.assertEqual(report(),json.loads((ROOT/'docs/ribbon-command-validation.json').read_bytes()))

    def test_missing_rows_relaxed_bounds_and_false_support_claims_are_rejected(self):
        c,log=self.inputs()
        with self.assertRaises(AssertionError):summarize(c,log.replace('RIBBON_COMMAND ','MISSING ',1))
        for key,value,want_gap in [('image_bound',1e-5,False),('full_command_parity',True,False),('local_supported',True,True)]:
            lines=[];changed=False
            for line in log.splitlines():
                if not changed and 'RIBBON_COMMAND 'in line:
                    prefix,text=line.split('RIBBON_COMMAND ',1);row=json.loads(text)
                    if row['local_supported']is(not want_gap):
                        row[key]=value;line=prefix+'RIBBON_COMMAND '+json.dumps(row);changed=True
                lines.append(line)
            self.assertTrue(changed)
            with self.assertRaises(AssertionError):summarize(c,'\n'.join(lines))

    def test_unknown_native_command_is_rejected(self):
        c,log=self.inputs();c=copy.deepcopy(c)
        c['ribbon_workflows'][1]['results'][0]['value']['history']+='\nUnknown command: _Yes'
        with self.assertRaises(AssertionError):summarize(c,log)


if __name__=='__main__':unittest.main()
