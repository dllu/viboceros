"""Audit native Shell coverage, real history and finite comparison gates."""
import copy
import gzip
import json
from pathlib import Path
import shutil
import tempfile
import unittest
from . import shell_native_evidence as evidence, report_shell_native as report
from .check_shell_native import ROOT, PROOF
from .fillet_zero_history import audit


class ShellNativeTests(unittest.TestCase):
    def test_every_capture_diagnostic_and_failed_attempt_is_accounted_for(self):
        rows,sdk,signed,captures,counts,paths=evidence.collect(PROOF)
        self.assertEqual((len(rows),len(sdk),len(signed),len(captures)),(25,9,6,5))
        self.assertEqual(counts['qualified_commands'],24)
        self.assertEqual(counts['physical_picks'],22)
        self.assertEqual(counts['retained_failed_attempts'],5)
        self.assertGreater(len(paths),40)
        self.assertEqual(len(report.portable(rows)),12)

    def test_native_replacements_copies_zero_skins_and_selection_are_distinct(self):
        rows=evidence.collect(PROOF)[0]
        keep=rows['shell_q_keep'];delete=rows['shell_q_delete']
        self.assertEqual(len(keep['after']),3);self.assertEqual(len(delete['after']),2)
        self.assertFalse(keep['after'][-1]['guide']);self.assertTrue(delete['after'][-1]['guide'])
        self.assertEqual(keep['after'][-1]['attributes'],keep['before'][-1]['attributes'])
        self.assertEqual(delete['after'][-1]['attributes'],delete['before'][-1]['attributes'])
        zero=rows['shell_q_zero_delete']['after'][-1]['brep']
        self.assertEqual((len(zero['faces']),len(zero['edges']),len(zero['vertices'])),(10,20,8))
        self.assertTrue(zero['topology']['solid'])
        self.assertTrue(all(o['selected']==0 for v in rows.values() for o in v['after']))

    def test_sdk_sign_discovery_keeps_open_negative_results(self):
        signed=evidence.collect(PROOF)[2]
        for key,v in signed.items():
            negative,positive=v['attempts']
            self.assertEqual([negative['distance'],positive['distance']],[-1.,1.])
            self.assertEqual(len(negative['breps']),4 if key=='shell_disconnected' else 2)
            self.assertTrue(all(b['native_valid'] and not b['is_solid'] for b in negative['breps']))
            self.assertTrue(all(b['native_valid'] and b['is_solid'] for b in positive['breps']))

    def test_history_requires_real_complete_undo_redo_pairs(self):
        rows=evidence.collect(PROOF)[0];value=rows['shell_q_zero_delete']
        self.assertEqual(audit(value,'Shell'),dict(recorded=True,geometry_changed=True))
        broken=copy.deepcopy(value)
        broken['undo_observation']['undo_signals']=[s for s in broken['undo_observation']['undo_signals'] if not s['end_undo']]
        with self.assertRaisesRegex(ValueError,'incomplete UndoRedo'): audit(broken,'Shell')
        self.assertEqual(audit(rows['shell_q_picked_cancel'],'Shell'),dict(recorded=False,geometry_changed=False))

    def test_failed_receipt_cannot_be_published_as_a_successful_capture(self):
        with tempfile.TemporaryDirectory(dir=ROOT/'docs') as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof)
            p=proof/'captures.json';receipts=json.loads(p.read_bytes());receipts[0]['exit_status']=1
            p.write_text(json.dumps(receipts))
            with self.assertRaisesRegex(ValueError,'headless receipt'): evidence.collect(proof)

    def test_missing_duplicate_nonfinite_and_excessive_comparisons_fail(self):
        if not (PROOF/'command-native.log.gz').exists(): self.skipTest('checks follow implementation')
        with tempfile.TemporaryDirectory(dir=ROOT/'docs') as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof)
            p=proof/'command-native.log.gz';lines=gzip.decompress(p.read_bytes()).decode().splitlines(keepends=True)
            index=next(i for i,line in enumerate(lines) if line.startswith('SHELL_COMMAND '))
            row=json.loads(lines[index].split(' ',1)[1])
            for value in [float('nan'),-1.,1.1e-6]:
                changed=lines.copy();changed[index]='SHELL_COMMAND '+json.dumps(dict(row,maximum_geometry_error=value))+'\n'
                p.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
                with self.assertRaisesRegex(ValueError,'geometry gate'): report.comparisons(proof)
            for changed in [lines[:index]+lines[index+1:],lines+[lines[index]]]:
                p.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
                with self.assertRaisesRegex(ValueError,'incomplete or duplicate'): report.comparisons(proof)

    def test_pinned_record_reproduces(self):
        path=ROOT/'docs/shell-native-validation.json'
        if not path.exists(): self.skipTest('record follows implementation commit')
        record=json.loads(path.read_bytes())
        self.assertEqual(record,report.build(PROOF,record['implementation_revision']))


if __name__=='__main__': unittest.main()
