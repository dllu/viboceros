"""Keep source approximation, native output bounds and macro settings distinct."""
import copy
import json
import unittest
from . import report_curve_loose_offset as record


def replace(log,prefix,rows):
    it=iter(rows)
    return '\n'.join(prefix+' '+json.dumps(next(it))if s.startswith(prefix+' ')else s for s in log.splitlines())


class LooseOffsetRecordTests(unittest.TestCase):
    def test_saved_record_reproduces(self):
        self.assertEqual(record.report(),json.loads((record.ROOT/'docs/curve-loose-offset-validation.json').read_bytes()))

    def test_native_gate_and_distinct_target_matching_are_required(self):
        corpus=[record.captures(f)for f in record.FAMILIES]
        log=record.read(record.PROOF/'portable.log.gz').decode()
        original=record.rows(log,'CURVE_LOOSE_OFFSET')
        for key,value in [('native_bound',1e-8),('native_controls',99),('native_index',99)]:
            changed=copy.deepcopy(original);changed[0][key]=value
            with self.assertRaises((AssertionError,IndexError)):
                record.summarize(corpus,replace(log,'CURVE_LOOSE_OFFSET',changed))
        changed=copy.deepcopy(corpus);changed[0][1]['results'][0]['value']['history']='Unknown command: _No'
        with self.assertRaises(AssertionError):record.summarize(changed,log)

    def test_effective_native_options_cannot_be_inferred_from_intended_tokens(self):
        q,n=record.captures('curve_loose_offset')
        op=q['operations'][0];v=n['results'][0]['value']
        record.settings(op,v)
        changed=copy.deepcopy(v);changed['history']=changed['history'].replace('Loose=Yes','Loose=No')
        with self.assertRaises(AssertionError):record.settings(op,changed)
        log=record.read(record.PROOF/'portable.log.gz').decode();rows=record.rows(log,'RIBBON_LOOSE_BOUNDARY')
        rows[0]['complete_ribbon_command']=True
        with self.assertRaises(AssertionError):
            record.summarize([record.captures(f)for f in record.FAMILIES],replace(log,'RIBBON_LOOSE_BOUNDARY',rows))


if __name__=='__main__':unittest.main()
