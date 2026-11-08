"""Capture retention, finite replay gates and revision-pinned qualification."""
import gzip
import json
import shutil
import tempfile
import unittest
from . import offset_srf_trims_evidence as evidence, report_offset_srf_trims as report
from .check_offset_srf_trims import ROOT, PROOF

class TrimOffsetTests(unittest.TestCase):
    def test_native_build_product_errors_are_distinct_from_reference_errors(self):
        cases={'one':dict(command_native_maximum_field_error=.4,
                command_native_native_maximum_field_error=.2),
            'two':dict(ui_portable_maximum_field_error=.3,
                ui_portable_native_maximum_field_error=.1)}
        self.assertEqual(report.maximum_error(cases,'maximum_field_error'),.4)
        self.assertEqual(report.maximum_error(cases,'native_maximum_field_error'),.2)

    def test_captures_failures_and_input_diagnostics_are_retained(self):
        rows,captures,counts,_=evidence.collect()
        self.assertEqual((len(rows),len(captures)),(39,5))
        self.assertEqual(counts,dict(observations=39,Success=32,Cancel=7,qualified=36,diagnostics=3,failed_attempts=1))
        self.assertTrue(rows['offset_tef_delete_postselect']['history_changed'])
        self.assertNotIn('Unknown command:',rows['offset_tef_delete_postselect']['history'])
        self.assertTrue(rows['offset_te_cancel']['pending_redo'])

    def test_altered_capture_receipt_is_rejected(self):
        with tempfile.TemporaryDirectory(dir=ROOT/'docs') as directory:
            proof=ROOT/directory/'proof'; shutil.copytree(PROOF,proof)
            path=proof/'captures.json'; receipts=json.loads(path.read_bytes())
            receipts[1]['command'][0]='desktop-launcher'
            path.write_text(json.dumps(receipts))
            with self.assertRaisesRegex(ValueError,'headless capture receipt'): evidence.collect(proof)

    def test_missing_or_nonfinite_replay_is_rejected(self):
        if not (PROOF/'trim-command-native.log.gz').exists(): self.skipTest('replays follow implementation')
        with tempfile.TemporaryDirectory(dir=ROOT/'docs') as directory:
            proof=ROOT/directory/'proof'; shutil.copytree(PROOF,proof)
            path=proof/'trim-command-native.log.gz'; lines=gzip.decompress(path.read_bytes()).decode().splitlines(keepends=True)
            i=next(i for i,line in enumerate(lines) if line.startswith('TRIM_OFFSET_COMMAND '))
            row=json.loads(lines[i].split(' ',1)[1])
            for key,value in [('maximum_geometry_error',float('nan')),('maximum_geometry_error',-1.),('maximum_field_error',1.)]:
                changed=lines.copy(); changed[i]='TRIM_OFFSET_COMMAND '+json.dumps(dict(row,**{key:value}))+'\n'
                path.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
                with self.assertRaisesRegex(ValueError,'geometry or fit gate'): report.comparisons(proof)
            for changed in [lines[:i]+lines[i+1:],lines+[lines[i]]]:
                path.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
                with self.assertRaisesRegex(ValueError,'incomplete or duplicate'): report.comparisons(proof)
            changed=lines.copy()
            changed[i]='TRIM_OFFSET_COMMAND '+json.dumps(dict(row,native_reference_tolerance=1.))+'\n'
            path.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
            with self.assertRaisesRegex(ValueError,'reference admission budget'): report.comparisons(proof)

    def test_record_reproduces(self):
        path=ROOT/'docs/offset-srf-trims-validation.json'
        if not path.exists(): self.skipTest('record follows implementation commit')
        value=json.loads(path.read_bytes())
        self.assertEqual(value,report.build(PROOF,value['implementation_revision']))

    def test_each_split_native_command_run_must_pass(self):
        for name in ['command-native','trim-command-native']:
            path=PROOF/(name+'.log.gz')
            if not path.exists(): self.skipTest('replays follow implementation')
            with tempfile.TemporaryDirectory(dir=ROOT/'docs') as directory:
                proof=ROOT/directory/'proof'; shutil.copytree(PROOF,proof)
                altered=proof/path.name
                text=gzip.decompress(path.read_bytes()).decode().replace('test result: ok.','test result: FAILED.')
                altered.write_bytes(gzip.compress(text.encode(),mtime=0))
                with self.assertRaisesRegex(ValueError,'failed replay'): report.comparisons(proof)

if __name__=='__main__': unittest.main()
