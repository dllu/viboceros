"""Native inventory, immutable producers, history and qualification gates."""
import copy
import gzip
import json
import shutil
import tempfile
import unittest
from . import offset_srf_loose_evidence as evidence,report_offset_srf_loose as report
from .check_offset_srf_loose import ROOT,PROOF
from .fillet_zero_history import audit

class LooseOffsetTests(unittest.TestCase):
    def test_complete_capture_inventory_and_failed_source_are_retained(self):
        rows,captures,counts,paths=evidence.collect()
        self.assertEqual((len(rows),len(captures),len(paths)),(48,6,61))
        self.assertEqual(counts,dict(observations=48,Success=44,qualified=44,diagnostics=4,Cancel=4))
        self.assertEqual(set(evidence.DIAGNOSTICS),{k for k,v in rows.items()if 'Unknown command:'in v['history']})

    def test_closed_solids_replace_independently_of_hidden_delete_preference(self):
        rows=evidence.collect()[0]
        for delete in ['keep','delete']:
            primer=rows['offset_ls_prime_'+delete]
            self.assertEqual(audit(primer,'OffsetSrf'),dict(recorded=False,geometry_changed=False))
            for shape in ['sphere','periodic']:
                v=rows['offset_ls_'+shape+'_'+delete]
                self.assertEqual(len(v['after']),2);self.assertTrue(v['after'][1]['guide'])
                self.assertEqual(v['after'][1]['face_count'],2)
                self.assertEqual(v['after'][1]['attributes'],v['before'][1]['attributes'])
                self.assertEqual(audit(v,'OffsetSrf'),dict(recorded=True,geometry_changed=True))

    def test_zero_loose_walls_and_authored_recipe_gate(self):
        rows=evidence.collect()[0]
        for key in ['offset_lf_zero_solid','offset_lf_zero_both_solid','offset_lt_plane_zero_solid','offset_lt_trim_zero_solid']:
            self.assertEqual(len(rows[key]['after']),3);self.assertEqual(rows[key]['after'][-1]['face_count'],2)
        v=rows['offset_lf_periodic_uv']['prescribed_case'];changed=copy.deepcopy(v)
        changed['curves'][0]['control_points'][0]['point'][0]+=1e-8
        self.assertFalse(evidence.authored_equal(v,changed))

    def test_failed_or_incomplete_headless_receipt_is_rejected(self):
        with tempfile.TemporaryDirectory(dir=ROOT/'docs')as directory:
            proof=ROOT/directory/'proof';shutil.copytree(PROOF,proof)
            p=proof/'captures.json';receipts=json.loads(p.read_bytes());receipts[0]['exit_status']=1;p.write_text(json.dumps(receipts))
            with self.assertRaisesRegex(ValueError,'headless capture receipt'):evidence.collect(proof)

    def test_missing_duplicate_or_nonfinite_comparison_is_rejected(self):
        if not (PROOF/'command-native.log.gz').exists():self.skipTest('checks follow implementation')
        with tempfile.TemporaryDirectory(dir=ROOT/'docs')as directory:
            proof=ROOT/directory/'proof';shutil.copytree(PROOF,proof);p=proof/'command-native.log.gz'
            lines=gzip.decompress(p.read_bytes()).decode().splitlines(keepends=True)
            index=next(i for i,line in enumerate(lines)if line.startswith('LOOSE_OFFSET_COMMAND '));row=json.loads(lines[index].split(' ',1)[1])
            for value in [float('nan'),-1.,1.1e-6]:
                changed=lines.copy();changed[index]='LOOSE_OFFSET_COMMAND '+json.dumps(dict(row,maximum_geometry_error=value))+'\n';p.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
                with self.assertRaisesRegex(ValueError,'geometry gate'):report.comparisons(proof)
            for changed in [lines[:index]+lines[index+1:],lines+[lines[index]]]:
                p.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
                with self.assertRaisesRegex(ValueError,'incomplete or duplicate'):report.comparisons(proof)

    def test_pinned_record_reproduces(self):
        path=ROOT/'docs/offset-srf-loose-validation.json'
        if not path.exists():self.skipTest('record follows implementation commit')
        record=json.loads(path.read_bytes());self.assertEqual(record,report.build(PROOF,record['implementation_revision']))

if __name__=='__main__':unittest.main()
