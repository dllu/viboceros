import copy
import json
import unittest
from . import report_ribbon_ruled as record

class RibbonRecordTests(unittest.TestCase):
    def test_record_reproduces(self):
        self.assertEqual(record.report(),json.loads((record.ROOT/'docs/ribbon-ruled-validation.json').read_bytes()))

    def test_geometry_bounds_cannot_relax_or_claim_command_parity(self):
        q=json.loads((record.ROOT/'tools/rhino_oracle/fixtures/ribbon_ruled.json').read_bytes())
        n=json.loads(record.read(record.ROOT/'tools/rhino_oracle/observations/ribbon_ruled.json.gz'))
        log=record.read(record.PROOF/'portable.log.gz').decode();original=record.rows(log)
        for key,value in [('support_coverage_bound',1e-8),('full_ribbon_command',True),('archive_bound',None)]:
            changed=copy.deepcopy(original);changed[0][key]=value;it=iter(changed)
            altered='\n'.join('RIBBON_RULED '+json.dumps(next(it))if s.startswith('RIBBON_RULED ')else s for s in log.splitlines())
            with self.assertRaises(AssertionError):record.summarize(q,n,altered)

    def test_the_known_incidence_gap_cannot_be_reported_as_a_match(self):
        q=json.loads((record.ROOT/'tools/rhino_oracle/fixtures/ribbon_ruled.json').read_bytes())
        n=json.loads(record.read(record.ROOT/'tools/rhino_oracle/observations/ribbon_ruled.json.gz'))
        log=record.read(record.PROOF/'portable.log.gz').decode();changed=record.rows(log)
        next(r for r in changed if r['id']=='ribbon-native-polycurve-both')['incidence_bound']=0.
        it=iter(changed);altered='\n'.join('RIBBON_RULED '+json.dumps(next(it))if s.startswith('RIBBON_RULED ')else s for s in log.splitlines())
        with self.assertRaises(AssertionError):record.summarize(q,n,altered)

if __name__=='__main__':unittest.main()
